# Vela protocol specification v0.10.0

This document defines the shipped v0 language kernel for portable,
correctable frontier state. It is normative for finding bundles, typed links,
proposal records, canonical events, proof freshness, content addressing,
frontier epistemic confidence, entity resolution, proof packets, and
Git-compatible storage.

Runtime objects, federation, and dedicated constellation interfaces are not part
of the v0 protocol contract.

## 1. design principles

1. **Narrow waist.** The substrate solves persistent, correctable scientific
   state.
2. **Finding first.** The paper is a source artifact. The finding bundle is the
   primary state object.
3. **State transition first.** Truth-changing writes become proposals and then
   canonical events.
4. **Disagreement is structural.** Contradictions and contested claims remain
   inspectable instead of being flattened.
5. **Git-compatible.** Frontiers can be versioned with normal files, commits,
   branches, and diffs. Network propagation remains outside v0.
6. **Content-addressed.** Stable content produces stable IDs.
7. **Correction over deletion.** Corrections preserve history.
8. **Agent output is source, not truth.** Agent traces, synthetic reports, and
   benchmark outputs require source/evidence/condition grounding and review.

## 2. primitive set

Vela v0 has three protocol-level primitives:

- **Object:** the finding bundle is the only state-changing public object type.
- **Link:** typed relationships between findings.
- **Event:** the authoritative state-transition record.

A frontier snapshot is a bounded, reviewable frontier state over a scientific
question. It is not a claim of final truth. ("Belief state" is the theory-side
nomenclature for the same object — see `docs/MATH.md` and `docs/THEORY.md`.)

### 2.1 finding bundle

A finding bundle is one assertion plus its evidence, conditions, entities,
confidence, provenance, flags, annotations, attachments, and links.

Required fields:

| Field | Meaning |
|-------|---------|
| `id` | `vf_...` content address |
| `version` | Finding schema version |
| `previous_version` | Previous finding ID if corrected |
| `assertion` | The bounded claim text and type |
| `evidence` | Evidence class, method, spans, model system, and statistics |
| `conditions` | Scope boundaries such as species, assay, comparator, endpoint |
| `confidence` | Frontier epistemic support and components |
| `provenance` | Source and extraction metadata |
| `flags` | Review-relevant state such as gap, contested, retracted |
| `links` | Typed relationships to other findings |
| `annotations` | Lightweight notes or caveats |
| `created` / `updated` | Timestamps |

### 2.2 source, evidence, and condition projections

Frontier snapshots may carry derived projections:

- `sources`: source artifacts such as papers, PDFs, JATS files, CSV rows,
  notes, agent traces, benchmark outputs, notebook entries, experiment logs, and
  synthetic reports.
- `evidence_atoms`: exact source-grounded units bearing on one finding.
- `condition_records`: materialized condition boundaries for one finding.

These projections support proof and review. They are not authoritative
transition logs. Older frontiers may omit them; the reference implementation can
derive them for check/export/proof and materialize them during normalize without
rewriting canonical events. In v0, writable normalization is a pre-transition
repair step: once canonical events exist, further durable changes should be
represented as reviewed state transitions instead of post hoc normalization.

### 2.3 reserved concepts

Future layers may add `ProtocolRecord`, `ExperimentRecord`, `ResultRecord`,
first-class `Observation`, runtime writeback, network propagation, and dedicated
constellation interfaces. They are outside v0 until they have replay, writeback,
proof/export, and review/merge semantics.

## 3. content addressing

Finding IDs are computed from content:

```text
SHA-256(normalize(assertion.text) + "|" + assertion.type + "|" + provenance_id)
```

`provenance_id` is `doi`, then `pmid`, then source title. The ID is `vf_` plus
the first 16 hex characters of the hash.

Source records use `vs_...`, evidence atoms use `vea_...`, condition records use
`vcnd_...`, proposals use `vpr_...`, and canonical events use `vev_...`.

## 4. confidence

`confidence.score` means bounded frontier epistemic support for the finding as
currently represented. It is not extraction accuracy, not truth probability, and
not review consensus by itself.

Vela keeps three notions separate:

- `confidence.score`: frontier epistemic support.
- `confidence.extraction_confidence`: extraction accuracy confidence.
- review state: proposals, canonical events, contestation flags, and signals.

The computed score uses:

```text
score = evidence_strength * replication_strength * model_relevance * sample_strength
        - review_penalty + calibration_adjustment
```

The normalized component names are `evidence_strength`,
`replication_strength`, `sample_strength`, `model_relevance`, `review_penalty`,
and `calibration_adjustment`. Legacy component names may be accepted on input
for compatibility.

## 5. links

Core v0 link types:

| Type | Meaning |
|------|---------|
| `supports` | Source finding provides evidence for target |
| `contradicts` | Findings oppose each other under comparable or overlapping conditions |
| `extends` | Source builds on or broadens target |
| `depends` | Source validity depends on target |
| `replicates` | Source independently reproduces target |
| `supersedes` | Source replaces target |
| `synthesized_from` | Source was compiled from one or more targets |

Links may include confidence, notes, evidence spans, conditional text, and
inference provenance. Link-derived outputs are review surfaces unless accepted
through normal frontier review.

### v0.10 — domain-neutral enum extensions

The first non-bio frontier published to the public hub (a particle-astrophysics
WIMP direct-detection frontier) surfaced that the v0 enum sets were
biology-leaning. v0.10 added domain-neutral entries — additively — without
changing content addressing for pre-v0.10 frontiers:

- **Entity type:** `particle` (WIMPs, photons), `instrument` (XENONnT, JWST —
  capital objects that run measurements), `dataset` (instrument data releases
  distinct from the paper that reports them), `quantity` (named numerical
  values with units, e.g. `28 GeV/c^2`). The pre-v0.10 entries (`gene`,
  `protein`, …) and the `other` escape valve remain.
- **Assertion type:** `measurement` (numerical-quantity reports), `exclusion`
  (upper/lower bound at a confidence level). Pre-v0.10 entries unchanged.
- **Provenance source type:** `data_release` (instrument runs, observation
  campaigns, dataset versions that are themselves the substantive object).
  Pre-v0.10 entries unchanged.

Schema URL bumps `v0.8.0 → v0.10.0` for new frontiers; the validator accepts
both URLs so pre-v0.10 frontiers (BBB, BBB-extension, the v0.8 cross-frontier
conformance vector) replay byte-identically under a v0.10 binary.

### v0.8 — cross-frontier link targets

`Link.target` may take two shapes:

- `vf_<16hex>` — references a finding in this same frontier.
- `vf_<16hex>@vfr_<16hex>` — references a finding in a different frontier
  (the trailing `vfr_` is the target frontier's content-addressed id).

Cross-frontier targets are valid only if the dependent frontier declares a
matching `vfr_id` in `frontier.dependencies` with both a `locator` and a
`pinned_snapshot_hash`. Strict validation refuses cross-frontier targets
without a declared dep.

`vela registry pull <vfr> --transitive` walks the dependency graph and
verifies that every fetched dep's actual snapshot matches the dependent's
pinned hash. The pin is the integrity guarantee; partial trust is not a
state v0.8 supports.

## 6. proposal and event protocol

The public write boundary is a `vela.proposal.v0.1` proposal. Truth-changing
commands create pending proposals by default. `--apply` accepts and applies the
proposal locally in one step.

Accepted proposals append a canonical `StateEvent`, apply the reducer,
recompute derived state, and mark proof stale when appropriate.

Core proposal kinds:

| Kind | CLI surface |
|------|-------------|
| `finding.add` | `vela finding add` |
| `finding.review` | `vela review` |
| `finding.note` | `vela note` |
| `finding.caveat` | `vela caveat` |
| `finding.confidence_revise` | `vela revise` |
| `finding.reject` | `vela reject` |
| `finding.retract` | `vela retract` |

Core event kinds:

| Kind | Meaning |
|------|---------|
| `finding.asserted` | Add a finding |
| `finding.reviewed` | Record review judgment |
| `finding.noted` | Attach a note |
| `finding.caveated` | Attach a caveat |
| `finding.confidence_revised` | Revise confidence interpretation |
| `finding.rejected` | Mark a finding rejected |
| `finding.retracted` | Mark retraction state |

Canonical `events` are the authoritative write log. Legacy `review_events` and
`confidence_updates` fields may be read for compatibility, but new v0 writes
should not rely on them as state authority.

The non-normative [State Transition Spec](STATE_TRANSITION_SPEC.md) sketches the
larger typed transition language this protocol can grow into. v0 remains
proposal/event/finding centered.

## 7. storage layout

The portable baseline is monolithic `frontier.json`.

A `.vela` repository may also store frontier state as files:

```text
.vela/
  config.toml
  findings/
    vf_{hash}.json
  events/
    vev_{hash}.json
  proposals/
    vpr_{hash}.json
  proof-state.json
```

Older repositories may include split link manifests, review projection files,
confidence-update projection files, runs, or trails. Those are compatibility or
roadmap artifacts, not required v0 public storage.

## 8. proof packet contract

`vela proof` exports a review packet from frontier state without modifying the
input frontier by default. `--record-proof-state` may be used for local
bookkeeping after successful packet validation. Required packet families
include:

- manifest, overview, scope, packet lock, and RO-Crate metadata
- full findings
- source registry, evidence atoms, and source/evidence map
- condition records and condition matrix
- candidate gaps, bridges, tensions, review queue, and signals
- canonical events and replay report
- proposals
- proof trace

`packet validate` checks packet integrity. Proof freshness relative to later
accepted frontier writes is tracked in frontier state when proof state has been
recorded.

## 9. derived signals

Signals are recomputed from frontier state. They include proof readiness, review
queues, candidate gaps, candidate bridges, candidate tensions, observer-policy
rerankings, and simulated retraction impact over declared dependency links.

Signals are not standalone scientific facts.

## 10. conformance

A conforming v0 implementation must:

1. Read and write finding bundles matching `finding-bundle.v0.2.0.json`.
2. Generate content-addressed IDs using the v0 pre-image rules.
3. Compute confidence from structured evidence fields.
4. Preserve source/evidence/condition boundaries.
5. Preserve disagreement through typed links and review state.
6. Use proposal-first writes for truth-changing state changes.
7. Store canonical events as the authoritative transition log.
8. Validate replay and proof freshness for proof-facing output.
9. Support monolithic frontier JSON and Git-compatible `.vela` layout.
10. Preserve read compatibility for legacy review/confidence fields where
    practical.

A conforming implementation should expose machine-readable check/proof/serve
contracts and keep candidate signals caveated.

## 11. Non-Normative roadmap boundary

The larger theory includes runtime, network propagation, and constellated
coordination. Vela v0 only standardizes the state kernel. Future object families
or network behavior must be promoted through the same discipline: replay,
writeback, proof/export, review, and merge semantics first.

---

*Vela Protocol Specification v0.10.0 - April 2026*
