# Vela 2026 proof (Proof-Ready v0)

Vela’s proof-ready v0 claim is:

> A replayable frontier is a better working memory for AI-accelerated science
> than papers, logs, notebooks, and private agent memory alone.

This is intentionally narrow and conservative.

Vela v0 proves state, not extraction. `compile` can bootstrap candidate state
from artifacts, but the proof primitive is accepted frontier transition:
proposal -> canonical event -> reducer -> replayable state. The BBB frontier is
a protocol example, not an authoritative Alzheimer's BBB science map.

## What must be demonstrated

Within one real domain, Vela should show:

1. a bounded frontier can preserve typed findings, evidence, conditions, and provenance
2. evidence, dependencies, candidate tensions, and accepted contradictions become inspectable
3. reviewed corrections can change inherited frontier state
4. frontier state updates through review/correction history are visible and traceable
5. accepted changes are replayable through canonical events
6. later accepted changes can invalidate stale proof packets
7. humans or agents can write auditable review/correction events back into the frontier
8. agent-generated or machine-derived artifacts are treated as sources requiring
   provenance, conditions, caveats, and review before they change frontier state

## Proof artifact requirements

For BBB/Alzheimer proof readiness, the packet should include:

- a bounded frontier (`frontier.json`) with explicit source list and scope
- `sources/source-registry.json` identifying the artifacts that produced the
  frontier
- `evidence/evidence-atoms.json` and `evidence/source-evidence-map.json`
  linking exact spans, rows, measurements, or weak provenance atoms back to
  source records and finding IDs
- `conditions/condition-records.json` and `conditions/condition-matrix.json`
  preserving model system, species, method, comparator status,
  exposure/efficacy scope, and translation scope
- a short before/after comparison that shows inherited state changed
- one correction/update example where frontier state changes through proposal,
  event, reducer, and replay
- MCP workflow evidence with explicit finding IDs (`vf_xxx`) and caveats
- evidence atoms and confidence class for top claims (meta-analysis > RCT > cohort > case-control > case-report > in-vitro)
- `proof-trace.json` with source hash, canonical event log hash, proposal-state hash, replay status, schema version, checked artifacts, packet validation, and caveats
- `signals.json`, `review-queue.json`, and `quality-table.json` so reviewers can see frontier quality needs
- `events/events.json` and `events/replay-report.json` as the canonical change log and replay validation report
- `proposals/proposals.json` so reviewers can see pending, rejected, and applied proposal records
- `state-transitions.json`, `reviews/review-events.json`, and
  `reviews/confidence-updates.json` as compatibility projections so reviewers can inspect what changed
- `ro-crate-metadata.jsonld` for portable packet metadata

## Reviewer path

An external reviewer should be able to run:

```bash
vela stats frontiers/bbb-alzheimer.json
vela check frontiers/bbb-alzheimer.json
vela search "LRP1 RAGE amyloid" --source frontiers/bbb-alzheimer.json
cp frontiers/bbb-alzheimer.json /tmp/bbb-review-frontier.json
FINDING_ID=$(jq -r '.findings[0].id' /tmp/bbb-review-frontier.json)
vela review /tmp/bbb-review-frontier.json "$FINDING_ID" --status contested --reason "Reviewer found a missing caveat" --reviewer reviewer:demo --apply
vela history /tmp/bbb-review-frontier.json "$FINDING_ID"
vela tensions frontiers/bbb-alzheimer.json --both-high
vela gaps rank frontiers/bbb-alzheimer.json --top 5
vela proof frontiers/bbb-alzheimer.json --out proof-packet
vela packet validate proof-packet
```

`vela proof` is non-mutating by default. It computes proof state for the JSON
response but does not save it into the input frontier unless
`--record-proof-state` is passed.

The reviewer should inspect finding IDs, source evidence, confidence fields, and
candidate caveats before treating any output as an actionable scientific
judgment.

In proof review, `confidence.score` should be read as bounded frontier
epistemic support for the finding as currently represented. It is not a truth
label. Extraction reliability remains separate at
`confidence.extraction_confidence`, and contestation/review state remains in
proposals, canonical events, and signals.

Constellation language in the proof path stays narrow: current tensions, gaps,
bridges, review queues, and dependency/contradiction structure are early
mesoscale projections of frontier state. A dedicated constellation interface is
later work.

## Non-goals

Vela does not claim to prove:

- full institutional federation
- global protocol standardization
- wet-lab orchestration
- complete experiment/runtime ownership
- autonomous discovery
- full field-scale coverage in v0
- machine extraction as final validation
- compile quality as the release proof
- an authoritative BBB/Alzheimer science map

## Confluence

If a reviewer can inspect a bounded frontier, accept a correction, replay the
state, observe inherited state change, and see an old proof marked stale while
uncertainty remains explicit, the proof target is being met.

## Worked example: validated end-to-end (v0.14)

The proof contract is no longer hypothetical. The `Alzheimer's drug-target
landscape` frontier (`vfr_773f6e60b19b438f`) on the public hub is a
non-trivial real-curator artifact that exercises every protocol-§6 proposal
kind shipped through v0.14. Its proof packet round-trips clean:

```bash
vela registry pull vfr_773f6e60b19b438f \
  --from https://vela-hub.fly.dev/entries \
  --out ./will.json
vela proof ./will.json --out ./proof-will
vela packet validate ./proof-will
python3 scripts/cross_impl_conformance.py ./will.json
```

What's verified:

- **34 packet files generated**, including `manifest.json`, `events/events.json`,
  `events/replay-report.json`, `sources/source-registry.json`,
  `evidence/evidence-atoms.json`, `conditions/condition-records.json`,
  `proposals/proposals.json`, `proof-trace.json`, and the materialized
  finding/signals/quality projections.
- **`vela packet validate` → status: ok, checked_files: 34.**
- **Replay report:** `ok = true`, `status = ok`, 0 conflicts. The full
  16-event log replays into a frontier whose `snapshot_hash` matches the
  exported manifest.
- **Cross-impl conformance (`scripts/cross_impl_conformance.py`):** the
  Python implementation re-derives every `vf_…` (12), every `vev_…` (16),
  and every `vpr_…` (16) bit-identically from canonical JSON alone — strong
  evidence that the substrate's content-addressing rules are
  implementation-portable.
- **Event-kind coverage in this single packet:** `finding.asserted` (×11),
  `finding.caveated` (×2), `finding.noted` (×2), `finding.superseded` (×1).
  The v0.14 supersede event (`vev_c407a6443198bcf1` → new finding
  `vf_9b9311fd80f87572`) chains correctly through replay; the superseded
  finding carries `flags.superseded = true`; the new finding carries an
  auto-injected `supersedes` link back.
- **Source registry materialized inline (v0.13):** 11 `SourceRecord` entries
  populated by `proposals::create_or_apply` at apply-time, each carrying
  the structured provenance from the v0.11 `vela finding add --doi/--year/
  --journal/--source-authors` flags. 12 evidence atoms and 12 condition
  records derived from the same projection.

This is the substrate doctrine landing. A third party can pull, verify
hashes, replay events, validate the packet, and re-derive every
content-addressed id without trusting the publisher beyond the Ed25519
signature on the registry manifest. Two implementations agree
byte-for-byte on what the bytes mean.
