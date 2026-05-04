# Vela protocol primitives

This document is a language-neutral primer for the five protocol primitives
that external reducers, SDKs, and agents need to understand before writing
frontier state. The Rust implementation remains the reference implementation,
but this page describes the wire shape in canonical JSON terms.

The five primitives are:

- `Finding`: the durable claim object. Implemented as `FindingBundle` in
  [`crates/vela-protocol/src/bundle.rs`](../crates/vela-protocol/src/bundle.rs).
- `EvidenceObject`: source-grounded support for a finding. Implemented through
  inline `Evidence`, `Provenance`, source records, and evidence atoms.
- `Correction`: a reviewed state transition over a finding. Implemented as
  canonical `StateEvent` records.
- `Link`: a typed relationship from one finding to another.
- `Frontier`: the bounded, replayable state file. Implemented as `Project` in
  [`crates/vela-protocol/src/project.rs`](../crates/vela-protocol/src/project.rs).

Canonical JSON rules and event-kind semantics are defined on the public
[`/spec`](https://vela.science/spec) page. The v0 protocol overview is in
[`docs/PROTOCOL.md`](PROTOCOL.md).

## Common rules

- Object keys are serialized by the canonical JSON rules in `/spec#canonical`.
- Timestamps are RFC 3339 strings.
- IDs are content addresses when the object has stable hash rules.
- Unknown fields must not be invented by reducers. Preserve fields that the
  parser can carry, and reject fields only when the schema or reducer rule says
  they are invalid.
- Agent output is proposal input, not canonical truth. Durable state changes
  pass through proposals and canonical events.

## Finding

A `Finding` is one bounded assertion plus evidence, conditions, confidence,
provenance, review flags, annotations, attachments, and outgoing links.

Reference type: [`FindingBundle`](../crates/vela-protocol/src/bundle.rs)

### Example

```json
{
  "id": "vf_08c81dd507f6a047",
  "version": 1,
  "previous_version": null,
  "assertion": {
    "text": "LRP1 transports amyloid-beta across the blood-brain barrier under the reported assay conditions.",
    "type": "mechanism",
    "entities": [],
    "relation": null,
    "direction": "positive"
  },
  "evidence": {
    "type": "experimental",
    "model_system": "mouse",
    "species": "Mus musculus",
    "method": "in vivo transport assay",
    "sample_size": "n=24",
    "effect_size": "reported in source",
    "p_value": "p < 0.05",
    "replicated": false,
    "replication_count": 0,
    "evidence_spans": []
  },
  "conditions": {
    "text": "mouse blood-brain barrier transport model",
    "species_verified": ["mouse"],
    "species_unverified": [],
    "in_vitro": false,
    "in_vivo": true,
    "human_data": false,
    "clinical_trial": false,
    "concentration_range": null,
    "duration": null,
    "age_group": null,
    "cell_type": "brain endothelium"
  },
  "confidence": {
    "kind": "frontier_epistemic",
    "score": 0.72,
    "basis": "experimental evidence with model-system caveat",
    "method": "computed",
    "components": {
      "evidence_strength": 0.8,
      "replication_strength": 0.4,
      "sample_strength": 0.6,
      "model_relevance": 0.7,
      "review_penalty": 0.0,
      "calibration_adjustment": 0.0,
      "causal_consistency": 1.0,
      "formula_version": "v0.8"
    },
    "extraction_confidence": 0.85
  },
  "provenance": {
    "source_type": "published_paper",
    "doi": "10.1016/j.neuron.2013.10.061",
    "pmid": null,
    "pmc": null,
    "openalex_id": null,
    "url": null,
    "title": "Example source title",
    "authors": [],
    "year": 2013,
    "journal": "Neuron",
    "extraction": {},
    "review": null
  },
  "flags": {
    "gap": false,
    "negative_space": false,
    "contested": false,
    "retracted": false,
    "declining": false,
    "gravity_well": false,
    "review_state": null
  },
  "links": [
    {
      "target": "vf_171833bd31b24037",
      "type": "supports",
      "note": "Same mechanism under related conditions.",
      "inferred_by": "compiler",
      "created_at": "2026-04-22T00:00:00Z"
    }
  ],
  "annotations": [],
  "attachments": [],
  "created": "2026-04-22T00:00:00Z",
  "updated": null
}
```

### Fields

| Field | Type | Status | Notes |
|---|---|---|---|
| `id` | string | REQUIRED | `vf_` plus 16 hex chars from the content-addressing preimage. |
| `version` | integer | REQUIRED | Defaults to `1` in the Rust type. |
| `previous_version` | string or null | REQUIRED | Previous finding ID when a corrected finding supersedes another. |
| `assertion` | object | REQUIRED | Claim text and assertion type. See `VALID_ASSERTION_TYPES`. |
| `evidence` | object | REQUIRED | Inline evidence summary. See EvidenceObject below. |
| `conditions` | object | REQUIRED | Scope boundaries such as species, model, tissue, dose, and timepoint. |
| `confidence` | object | REQUIRED | Frontier epistemic support, not truth probability. |
| `provenance` | object | REQUIRED | Source identity and extraction/review metadata. |
| `flags` | object | REQUIRED | Review and signal state such as `contested` and `retracted`. |
| `links` | array of `Link` | REQUIRED | Defaults to `[]`; outgoing typed links. |
| `annotations` | array | REQUIRED | Defaults to `[]`; notes and caveats materialized by events. |
| `attachments` | array | OPTIONAL | Extra artifacts attached to the finding. |
| `created` | RFC 3339 string | REQUIRED | Creation time. |
| `updated` | RFC 3339 string or null | REQUIRED | Last materialized update time, if any. |

### Mutating events

Finding state is changed by canonical event kinds listed in
[`docs/PROTOCOL.md#6-proposal-and-event-protocol`](PROTOCOL.md#6-proposal-and-event-protocol)
and `/spec#kinds`:

- `finding.asserted`: appends a new finding.
- `finding.reviewed`: changes review state and contested flags.
- `finding.noted`: appends an annotation.
- `finding.caveated`: appends a caveat annotation.
- `finding.confidence_revised`: updates confidence interpretation.
- `finding.rejected`: marks the finding as contested/rejected.
- `finding.retracted`: marks the finding retracted.

### Content addressing

Finding IDs are computed as:

```text
vf_ + first_16_hex(SHA-256(normalize(assertion.text) + "|" + assertion.type + "|" + provenance_id))
```

`normalize` lowercases text, collapses whitespace, and strips trailing
punctuation. `provenance_id` is `doi`, then `pmid`, then `title`.

## EvidenceObject

An `EvidenceObject` is source-grounded support, opposition, or context for a
finding. In v0 this is represented in three places:

- `Finding.evidence`: the inline evidence summary used by the finding.
- `Finding.provenance`: the source identity for the finding.
- `frontier.sources`, `frontier.evidence_atoms`, and `frontier.condition_records`:
  proof and review projections that point at exact source material.

Reference types: [`Evidence`](../crates/vela-protocol/src/bundle.rs),
[`Provenance`](../crates/vela-protocol/src/bundle.rs), and source records in
[`crates/vela-protocol/src/sources.rs`](../crates/vela-protocol/src/sources.rs).

### Example

```json
{
  "type": "experimental",
  "model_system": "mouse",
  "species": "Mus musculus",
  "method": "transport assay",
  "sample_size": "n=24",
  "effect_size": "reduced amyloid-beta clearance",
  "p_value": "p < 0.05",
  "replicated": false,
  "replication_count": 0,
  "evidence_spans": [
    {
      "span": "Figure 2 and associated results text",
      "direction": "supports"
    }
  ]
}
```

### Fields

| Field | Type | Status | Notes |
|---|---|---|---|
| `type` | string | REQUIRED | Serialized name for `evidence_type`. One of the valid evidence types, e.g. `experimental`, `observational`, `computational`. |
| `method` | string | REQUIRED | Method or study design summary. |
| `model_system` | string | REQUIRED | Assay, organism, cell line, cohort, or simulation context. Defaults to empty string. |
| `species` | string or null | OPTIONAL | Species when known. |
| `sample_size` | string or null | OPTIONAL | Known sample count in source form, e.g. `n=24`. |
| `effect_size` | string or null | OPTIONAL | Reported effect size or qualitative effect. |
| `p_value` | string or null | OPTIONAL | Reported p-value or significance string. |
| `replicated` | boolean | REQUIRED | Legacy scalar. New frontiers may also use first-class replication records. |
| `replication_count` | integer or null | OPTIONAL | Legacy scalar count. |
| `evidence_spans` | array | REQUIRED | Source spans, rows, figures, tables, or excerpts when available. |

### Mutating events

Evidence can enter state through `finding.asserted` as part of the finding
payload. Later review events may attach notes, caveats, confidence changes, or
source-grounded evidence projections. Legacy `review_events` and
`confidence_updates` may be present for compatibility, but canonical `events`
are the authoritative transition log for new writes.

### Content addressing

Inline `Finding.evidence` does not have its own public `vea_` ID. Evidence
atoms and source records use their own content-addressed IDs in projection
registries. A finding's `evidence` content contributes to the canonical finding
hash used in event replay, but not to the `vf_` ID derivation.

## Correction

A `Correction` is a reviewed state transition that changes interpretation of a
finding without deleting history. In v0, corrections are expressed as
`StateEvent` records: review, caveat, note, confidence revision, rejection, and
retraction events.

Reference type: [`StateEvent`](../crates/vela-protocol/src/events.rs)

### Example

```json
{
  "schema": "vela.event.v0.1",
  "id": "vev_0f4c2d5a8b9e10ab",
  "kind": "finding.confidence_revised",
  "target": {
    "type": "finding",
    "id": "vf_08c81dd507f6a047"
  },
  "actor": {
    "id": "reviewer:demo",
    "type": "human"
  },
  "timestamp": "2026-04-23T10:15:00Z",
  "reason": "Mouse-only evidence should carry a model-system caveat.",
  "before_hash": "sha256:old",
  "after_hash": "sha256:new",
  "payload": {
    "previous_score": 0.72,
    "new_score": 0.61
  },
  "caveats": [
    "Model-system evidence may not transfer directly to human BBB biology."
  ],
  "signature": null
}
```

### Fields

| Field | Type | Status | Notes |
|---|---|---|---|
| `schema` | string | REQUIRED | Defaults to `vela.event.v0.1`. |
| `id` | string | REQUIRED | Content-addressed event ID. |
| `kind` | string | REQUIRED | Event kind, e.g. `finding.confidence_revised`. |
| `target` | object | REQUIRED | Target object type and ID. |
| `actor` | object | REQUIRED | Actor ID and type. |
| `timestamp` | RFC 3339 string | REQUIRED | Event time. |
| `reason` | string | REQUIRED | Human-readable rationale. |
| `before_hash` | string | REQUIRED | Hash before the transition, or `sha256:null`. |
| `after_hash` | string | REQUIRED | Hash after the transition. |
| `payload` | object | REQUIRED | Kind-specific body. |
| `caveats` | array of strings | REQUIRED | Defaults to `[]`. |
| `signature` | string or null | OPTIONAL | Ed25519 signature when actor policy requires it. |

### Mutating events

The correction primitive is itself an event. Reducers apply the event by kind.
Current reducer paths include `finding.reviewed`, `finding.noted`,
`finding.caveated`, `finding.confidence_revised`, `finding.rejected`,
`finding.retracted`, and dependency-invalidation events.

### Content addressing

The event ID is derived from canonical JSON over the unsigned event shape. Event
logs are hashed with canonical JSON over the ordered event list. Finding hashes
used in `before_hash` and `after_hash` clear `links` before hashing so link
edits do not invalidate asserted-event replay.

## Link

A `Link` is a typed edge from the containing finding to another finding. Links
are review surfaces unless accepted through normal frontier review. They support
search, evidence-chain tracing, contradiction discovery, and retraction-impact
simulation.

Reference type: [`Link`](../crates/vela-protocol/src/bundle.rs)

### Example

```json
{
  "target": "vf_171833bd31b24037@vfr_093f7f15b6c79386",
  "type": "depends",
  "note": "This finding relies on the target's BBB transport mechanism.",
  "inferred_by": "reviewer:demo",
  "created_at": "2026-04-23T10:15:00Z"
}
```

### Fields

| Field | Type | Status | Notes |
|---|---|---|---|
| `target` | string | REQUIRED | `vf_<id>` for local links or `vf_<id>@vfr_<id>` for cross-frontier links. |
| `type` | string | REQUIRED | One of `supports`, `contradicts`, `extends`, `depends`, `replicates`, `supersedes`, `synthesized_from`. |
| `note` | string | REQUIRED | Human-readable rationale. |
| `inferred_by` | string | REQUIRED | Defaults to compiler metadata in the Rust type. |
| `created_at` | RFC 3339 string | REQUIRED | Defaults for backward compatibility on older frontiers. |
| `mechanism` | object | OPTIONAL | Optional causal mechanism annotation on relevant links, tagged by `kind`. |

### Mutating events

Links may be present in `finding.asserted` payloads and can be added through
link-related CLI surfaces. Link-derived outputs are candidate review surfaces
unless represented by accepted frontier state. Cross-frontier links are valid
only when the frontier declares the target `vfr_id` in `frontier.dependencies`.

### Content addressing

Links are part of the serialized finding object, but event replay hashes clear
links before computing finding hashes. This preserves replay validity when
relationships are added without changing the claim text itself. Cross-frontier
targets are integrity-bound by the dependency entry's pinned snapshot hash.

## Frontier

A `Frontier` is a bounded, reviewable body of scientific state. The portable
baseline is a monolithic `frontier.json`; a `.vela` repo may store the same
state as files. In Rust, the top-level frontier state is named `Project`.

Reference type: [`Project`](../crates/vela-protocol/src/project.rs)

### Example

```json
{
  "vela_version": "0.48.0",
  "schema": "https://vela.science/schema/finding-bundle/v0.10.0",
  "frontier_id": "vfr_093f7f15b6c79386",
  "frontier": {
    "name": "BBB Alzheimer frontier",
    "description": "Blood-brain barrier findings for Alzheimer's review.",
    "compiled_at": "2026-04-22T00:00:00Z",
    "compiler": "vela/0.48.0",
    "papers_processed": 10,
    "errors": 0,
    "dependencies": []
  },
  "stats": {
    "findings": 48,
    "links": 121,
    "replicated": 12,
    "unreplicated": 36,
    "avg_confidence": 0.742,
    "gaps": 7,
    "negative_space": 2,
    "contested": 4,
    "categories": {},
    "link_types": {},
    "human_reviewed": 3,
    "review_event_count": 0,
    "confidence_update_count": 0,
    "event_count": 11,
    "source_count": 0,
    "evidence_atom_count": 0,
    "condition_record_count": 0,
    "proposal_count": 0,
    "confidence_distribution": {
      "high_gt_80": 13,
      "medium_60_80": 28,
      "low_lt_60": 7
    }
  },
  "findings": [],
  "sources": [],
  "evidence_atoms": [],
  "condition_records": [],
  "review_events": [],
  "confidence_updates": [],
  "events": [],
  "proposals": [],
  "proof_state": {},
  "signatures": [],
  "actors": []
}
```

### Fields

| Field | Type | Status | Notes |
|---|---|---|---|
| `vela_version` | string | REQUIRED | Binary/protocol implementation version that wrote the file. |
| `schema` | string | REQUIRED | Schema URL accepted by the validator. |
| `frontier_id` | string or null | OPTIONAL | `vfr_` ID for new frontiers; legacy frontiers may omit it. |
| `frontier` | object | REQUIRED | Metadata: name, description, compile time, compiler, dependencies. |
| `stats` | object | REQUIRED | Derived counts and summary values, including confidence distribution and collection counts. Recomputed by the reference implementation. |
| `findings` | array of `Finding` | REQUIRED | Materialized finding state. |
| `sources` | array | OPTIONAL | Source registry projections. |
| `evidence_atoms` | array | OPTIONAL | Exact source-grounded evidence units. |
| `condition_records` | array | OPTIONAL | Materialized condition boundaries. |
| `review_events` | array | OPTIONAL | Legacy review event log. |
| `confidence_updates` | array | OPTIONAL | Legacy confidence update log. |
| `events` | array of `StateEvent` | REQUIRED | Canonical replay log for new writes. |
| `proposals` | array | REQUIRED | Pending/applied proposal records. |
| `proof_state` | object | REQUIRED | Frontier-local proof freshness projection; defaults to a `never_exported` latest packet. |
| `signatures` | array | REQUIRED | Signed envelopes for findings/events where present. |
| `actors` | array | OPTIONAL | Registered actor identities and keys. |
| `replications`, `datasets`, `code_artifacts`, `predictions`, `resolutions`, `peers` | arrays | OPTIONAL | Additive first-class kernel collections in later v0 releases. |

### Mutating events

A frontier changes when accepted proposals append canonical events and the
reducer materializes the next state. `frontier.created` is the genesis event for
new frontiers. Finding-level events mutate the `findings` collection. Actor,
registry, and dependency operations may update their corresponding frontier
collections.

### Content addressing

`frontier_id` is derived from the canonical hash of the `frontier.created`
genesis event when present. Legacy frontiers may derive a fallback ID from
metadata. Snapshot hashes use canonical JSON over the frontier state with
runtime-only event/signature/proof projections removed as defined in the
reference implementation. Cross-frontier dependency pins compare snapshot
hashes before satisfying `vf_...@vfr_...` links.

## Minimal reducer checklist

A new reducer or SDK should be able to implement the narrow read path with this
checklist:

1. Parse `frontier.json` as UTF-8 JSON.
2. Apply canonical JSON rules from `/spec#canonical`.
3. Load `frontier.events` in deterministic order.
4. Apply `finding.asserted`, `finding.reviewed`, `finding.noted`,
   `finding.caveated`, `finding.confidence_revised`, `finding.rejected`, and
   `finding.retracted`.
5. Recompute derived stats without changing canonical event bytes.
6. Recompute event-log and snapshot hashes.
7. Compare output bytes against the Rust, Python, and TypeScript reference
   reducers on the conformance fixtures.
