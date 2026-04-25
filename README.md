<p align="center">
  <img src="assets/brand/vela-logo-wordmark.svg" alt="Vela" height="44">
</p>

<p align="center">
  <em>A git-native protocol for replayable scientific frontier state.</em>
</p>

---

Vela turns a folder of papers into a bounded, reviewable state that a
scientist or an agent can correct, replay, and sign.

A *finding bundle* is the primary state object — an assertion with its
evidence, conditions, entities, confidence, and provenance. Findings live
inside a *frontier*: a bounded, reviewable frontier state over a scientific
question. Corrections enter as *proposals*, become canonical events on review,
and replay deterministically into the frontier. A *proof packet* seals the
current state so another party can re-verify it offline.

```bash
vela compile ./papers --output frontier.json
vela check frontier.json --strict --json
vela normalize frontier.json --out frontier.normalized.json
FINDING_ID=$(jq -r '.findings[0].id' frontier.json)
vela review frontier.normalized.json "$FINDING_ID" --status contested --reason "Mouse-only evidence" --reviewer reviewer:demo --apply
vela history frontier.normalized.json "$FINDING_ID"
vela proof frontier.normalized.json --out proof-packet
vela serve frontier.normalized.json
```

Vela does not claim to be a lab runtime, federation network, autonomous agent
loop, desktop app, or full science operating system. Those remain roadmap
or thesis items. v0 proves state, not extraction: scientific work can become
inspectable, correctable, replayable frontier state.

A public hub at <https://vela-hub.fly.dev> serves signed frontier manifests
over HTTPS. Anyone with an Ed25519 key can publish their own `vfr_id`. The
signature is the bind, not access control. See [docs/HUB.md](docs/HUB.md).

## What it does

- Bootstrap candidate frontier state from local corpora.
- Check frontier state before use as proof or agent context.
- Correct state through proposal-first reviews, caveats, revisions, rejections,
  finding additions, and retractions.
- Inspect findings with source, evidence, condition, confidence, provenance,
  links, proposals, and canonical event history.
- Surface candidate gaps, bridges, tensions, and review queues as derived
  signals.
- Export proof packets and serve the same state over MCP/HTTP.

Candidate gaps, bridges, tensions, observer rerankings, and PubMed prior-art
checks are review surfaces, not scientific conclusions.

## Quick start

```bash
cargo build --release -p vela-protocol
./target/release/vela compile examples/paper-folder/papers --output /tmp/frontier.json
./target/release/vela check /tmp/frontier.json --strict --json
./target/release/vela normalize /tmp/frontier.json --out /tmp/frontier.normalized.json
FINDING_ID=$(jq -r '.findings[0].id' /tmp/frontier.json)
./target/release/vela review /tmp/frontier.normalized.json "$FINDING_ID" --status contested --reason "Fixture review" --reviewer reviewer:demo --apply
./target/release/vela history /tmp/frontier.normalized.json "$FINDING_ID"
./target/release/vela proof /tmp/frontier.normalized.json --out /tmp/proof-packet
./target/release/vela serve /tmp/frontier.normalized.json --check-tools
```

`vela compile` writes `compile-report.json`, `quality-table.json`, and
`frontier-quality.md` beside the frontier. If no model key is configured, Vela
uses deterministic fallback extraction where possible and records that
limitation in the report. `compile` is onboarding, not the trust anchor:
reviewed and accepted state transitions are the boundary for frontier state.

For the checked-in BBB/Alzheimer sample:

```bash
vela stats frontiers/bbb-alzheimer.json
vela search "LRP1 RAGE amyloid" --source frontiers/bbb-alzheimer.json
vela tensions frontiers/bbb-alzheimer.json --both-high
vela gaps rank frontiers/bbb-alzheimer.json --top 5
vela proof frontiers/bbb-alzheimer.json --out /tmp/vela-proof-packet
```

`frontiers/bbb-alzheimer.json` is the canonical public BBB sample. It is a
protocol demo for state, review, replay, and proof mechanics, not a scientific
authority on Alzheimer's BBB delivery. The `projects/bbb-flagship/` directory is
source/workspace material for regenerating or inspecting that frontier, not a
second release artifact.
`vela proof` exports and validates a packet without modifying the input
frontier. Use `--record-proof-state` only for local bookkeeping when you want to
save the latest packet state back into that frontier.

For the shortest v0 proof narrative, run:

```bash
./demo/v0-state-proof-demo.sh
```

It works on a temporary copy of the BBB frontier and demonstrates a reviewed
correction becoming history, making the prior proof stale, and then refreshing a
proof packet for the corrected state.

## Core concepts

- **Frontier:** a bounded, reviewable frontier state over a scientific
  question.
- **Finding bundle:** the primary state object; an assertion with evidence,
  conditions, entities, confidence, provenance, and links.
- **Source:** the artifact a finding came from, such as a paper, dataset, note,
  agent trace, benchmark output, notebook entry, or log.
- **Evidence:** the exact span, row, table, measurement, run, metric, or weak
  provenance unit bearing on a finding.
- **Condition:** the boundary where a claim stops, including species, assay,
  comparator, exposure/efficacy scope, endpoint, and translation scope.
- **Confidence:** bounded frontier epistemic support for the finding as
  currently represented, not truth probability or extraction accuracy.
- **Canonical event:** the authoritative state-transition record.
- **Proposal:** the public write boundary for truth-changing corrections.
- **Proof packet:** a portable review artifact with packet manifest, source and
  evidence tables, signals, event/replay data, proposals, and proof trace.

See [Core Doctrine](docs/CORE_DOCTRINE.md) for the claim boundary.

## Proof target

The v0 proof claim is narrow:

> A replayable frontier is a better working memory for AI-accelerated science
> than papers, logs, notebooks, and private agent memory alone.

The BBB proof path must show that a correction becomes replayable frontier
state, changes what a human or agent inherits, and marks prior proof stale. See
[Proof](docs/PROOF.md).

## Documentation

- [Core Doctrine](docs/CORE_DOCTRINE.md) - canonical v0 claim boundary
- [First Frontier](docs/FIRST_FRONTIER.md) - first-user paper-folder workflow
- [Frontier Review](docs/FRONTIER_REVIEW.md) - correction and proposal workflow
- [Protocol](docs/PROTOCOL.md) - normative v0 state and event semantics
- [CLI JSON](docs/CLI_JSON.md) - machine-readable command envelopes
- [Proof](docs/PROOF.md) - proof packet contract and BBB proof target
- [Trace Format](docs/TRACE_FORMAT.md) - `proof-trace.json`
- [v0.2.0 Release Notes](docs/V0_RELEASE_NOTES.md) - evaluator-facing release summary
- [v0 Dogfood Report](docs/V0_DOGFOOD_REPORT.md) - internal first-frontier dogfood notes
- [MCP Setup](docs/MCP_SETUP.md) - MCP/HTTP serving
- [Benchmarks](docs/BENCHMARKS.md) - benchmark fixtures and drift checks
- [Eval Card](docs/EVAL_CARD.md) - evaluation posture
- [Theory](docs/THEORY.md) - non-normative theory appendix
- [Math](docs/MATH.md) - non-normative math and principles appendix
- [State Transition Spec](docs/STATE_TRANSITION_SPEC.md) - non-normative typed transition design bridge

## Status

- Rust workspace with the `vela` CLI and MCP/HTTP server in
  `crates/vela-protocol`
- canonical checked-in BBB/Alzheimer frontier under `frontiers/`
- local paper-folder compile path with quality reports
- proof packets, replay checks, proposal records, and proof freshness
- Apache-2.0

## Brand

Voice, color, and asset canon live in [docs/BRAND.md](docs/BRAND.md).
The static landing page at `web/index.html` is GitHub Pages deployable and
uses those tokens. Workbench preview HTMLs under `web/previews/` are proposed
product surfaces for post-v0 Vela — not shipping v0 product.
