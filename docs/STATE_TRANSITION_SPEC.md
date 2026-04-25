# State transition spec

This document is a non-normative design bridge between the v0 protocol and the
larger Vela theory. It names the shape of future typed state transitions without
claiming that every field is implemented today.

Vela v0 already implements the narrow kernel:

- finding bundles
- proposals
- canonical events
- reducers
- replayable frontier snapshots
- proof freshness

The broader design target is:

> Scientific artifacts become typed, replayable state transitions over bounded
> frontier state.

## Status legend

| Status | Meaning |
|--------|---------|
| Implemented | Present in the v0 public protocol or CLI behavior |
| Partial | Represented indirectly through current fields, links, events, or signals |
| Future | Theory-aligned but not a v0 release claim |

## Core transition

```ts
type ScientificStateTransition = {
  id: StateEventId
  schema: "vela.state-transition.v1"
  priorState: StateRef
  event:
    | EvidenceEvent
    | ReviewEvent
    | CorrectionEvent
    | RetractionEvent
    | ExperimentEvent
    | ProjectionEvent
  affectedObjects: ScientificObjectRef[]
  typedEffect:
    | "support"
    | "contradict"
    | "refine"
    | "weaken"
    | "retract"
    | "scope_change"
    | "projection_change"
  uncertaintyDelta: CredalUpdate
  provenance: ProvenanceTrace
  validityScope: Scope
  propagationObligations: UpdateObligation[]
}
```

Current mapping:

| Field | v0 status |
|-------|-----------|
| `id` | Implemented as `StateEvent.id` |
| `priorState` | Partial through event hashes and replay reports |
| `event` | Implemented for finding add/review/note/caveat/revise/reject/retract |
| `affectedObjects` | Partial through event target and typed links |
| `typedEffect` | Partial through event kind and link type |
| `uncertaintyDelta` | Partial through confidence revision and review state |
| `provenance` | Implemented for finding/source/evidence state; partial for events |
| `validityScope` | Implemented through condition records; partial in events |
| `propagationObligations` | Partial through simulated retraction impact |

## Scientific objects

```ts
type ScientificObjectRef =
  | FindingRef
  | SourceRef
  | EvidenceRef
  | ConditionRef
  | ContradictionRef
  | ProtocolRef
  | ExperimentRef
  | ResultRef
  | ProjectionRef
```

Current mapping:

- `FindingRef`: implemented as `vf_*`
- `SourceRef`: implemented through source registry records
- `EvidenceRef`: implemented through evidence atoms
- `ConditionRef`: implemented through condition records
- `ContradictionRef`: partial through typed links and tensions
- `ProtocolRef`, `ExperimentRef`, `ResultRef`: future runtime objects
- `ProjectionRef`: future constellation object

## Contradictions

Contradictions should remain live state, not disappear into one averaged
confidence score.

```ts
type Contradiction = {
  id: ContradictionId
  claims: [FindingRef, FindingRef]
  contradictionType:
    | "mechanistic"
    | "empirical"
    | "scope"
    | "methodological"
    | "statistical"
  suspectedResolutionVariables: Variable[]
  requiredResolution:
    | ExperimentDesign[]
    | ReviewAction[]
    | ScopeClarification[]
  status: "open" | "partially_resolved" | "resolved" | "reframed"
  provenance: ProvenanceTrace
}
```

Current mapping:

- typed contradiction links are implemented
- `vela tensions` surfaces candidate contradiction structure
- resolution variables and required experiments are future fields

Design rule:

> UI summaries may show scalar confidence, but protocol state should preserve
> contradiction structure and resolution conditions.

## Credal updates

Vela should eventually support imprecise probability rather than only scalar
confidence.

```ts
type CredalUpdate = {
  target: ScientificObjectRef
  previous:
    | ScalarConfidence
    | CredalSet
  next:
    | ScalarConfidence
    | CredalSet
  updateReason: string
  evidenceRefs: EvidenceRef[]
  scope: Scope
  caveats: string[]
}
```

Current mapping:

- scalar confidence is implemented
- confidence revisions are proposal-backed
- caveats and contested flags are implemented
- credal sets are future work

## Propagation obligations

A state transition can create obligations downstream.

```ts
type UpdateObligation = {
  id: ObligationId
  sourceTransition: StateEventId
  affectedObject: ScientificObjectRef
  reason:
    | "depends_on_retracted_finding"
    | "scope_changed"
    | "confidence_weakened"
    | "contradiction_opened"
    | "proof_stale"
  requiredAction:
    | "review"
    | "revise"
    | "retract"
    | "rerun_proof"
    | "rerun_benchmark"
  status: "open" | "accepted" | "rejected" | "resolved"
}
```

Current mapping:

- proof stale status is implemented
- simulated retraction propagation exists over declared links
- durable obligation records are future work

## Constellation projection

Constellations are projections over frontier state. They are not new truth
objects.

```ts
type ConstellationProjection = {
  id: ProjectionId
  frontier: FrontierRef
  observerPolicy: ObserverPolicy
  localViews: LocalScientificRegion[]
  gluingRules: CompatibilityRule[]
  obstructions: Obstruction[]
  actionFrontier: ExperimentOpportunity[]
}
```

```ts
type Obstruction = {
  id: ObstructionId
  kind:
    | "contradiction"
    | "scope_mismatch"
    | "method_translation_failure"
    | "missing_evidence"
    | "model_to_human_gap"
  localRegions: LocalScientificRegionRef[]
  affectedFindings: FindingRef[]
  requiredResolution: UpdateObligation[]
}
```

Current mapping:

- gaps, bridges, tensions, review queues, typed links, and observer rerankings
  are early projection surfaces
- local regions, gluing rules, and obstructions are future constellation work

## Implementation path

The theory should become software in this order:

1. Keep v0 stable: finding bundle -> proposal -> event -> reducer -> replay.
2. Make contradiction records first-class only after link/tension semantics are
   stable.
3. Add credal/uncertainty deltas without removing scalar confidence summaries.
4. Add propagation obligations as durable review work items.
5. Promote protocol/experiment/result records only with replay, proof/export,
   writeback, and review/merge semantics.
6. Build constellation projections as read-side projections over state, not as
   replacement truth objects.

## Non-Goals for v0

This spec does not require v0 to implement:

- first-class experiments
- autonomous runtime loops
- network federation
- BFT consensus
- formal sheaf machinery
- constellation UI

Those remain future layers over the state kernel.
