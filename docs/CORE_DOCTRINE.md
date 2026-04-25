# Core doctrine

Vela v0 is portable, correctable frontier state for bounded scientific
questions.

A frontier is a bounded, reviewable frontier state over a scientific question.
("Belief state" is the theory-side nomenclature; operational Vela uses
"frontier state." See `docs/MATH.md` and `docs/THEORY.md`.)
The finding bundle is the primary state object. The canonical event is the
primary change primitive.

The public release proves one thing: scientific activity can become frontier
state that humans and AI can inspect, correct, serve, replay, and export without
erasing uncertainty. Papers are the first bootstrap source. The product is the
state, not the papers, and v0 proves state rather than extraction quality.

Vela's job is to preserve the state those systems should inherit.

## Stable core

The release surface is intentionally small:

- `vela compile` bootstraps candidate state from a bounded literature query or
  input set into `frontier.json`.
- `vela check` validates frontier state and catches malformed proof artifacts.
- `vela review`, `vela revise`, `vela caveat`, `vela reject`, and
  `vela history` create, accept, and inspect proposal-backed state changes.
- `vela finding add` creates or applies a manual finding proposal when a human
  or agent needs to assert bounded state outside the compiler.
- `vela proof` exports a reviewable proof packet from frontier state.
- `vela serve` exposes frontier state over MCP/HTTP for structured inspection.

`compile` is an onboarding path, not the trust anchor. The trust boundary is the
accepted frontier transition: proposal -> canonical event -> reducer ->
replayable state.

Everything else is secondary to that path.

## Thesis

Science is becoming continuous, agentic, and high-throughput. Its memory system
is still episodic, prose-based, and publication-centered.

Vela is not another autonomous scientist. It is the state layer autonomous
scientists, literature agents, lab systems, benchmarks, and human teams should
write into. Agent traces, notebook entries, benchmark outputs, lab logs, and
papers can all be sources. None of them are truth by default.

The doctrine is simple:

1. The paper is not the atomic unit. The finding is.
2. Autonomous science needs shared frontier state.
3. A finding without conditions is incomplete.
4. A result without provenance is not evidence.
5. An agent trace is not truth.
6. A contradiction should become a reviewable state transition.
7. A failed experiment should become reusable terrain.
8. A review without typed consequence is only commentary.
9. A proof packet must be replayable.
10. Science must be able to change its mind at machine speed without forgetting why.

## Three layers

Vela v0 has one loop with three layers:

1. **Frontier state:** finding bundles, sources, evidence, entities, conditions,
   typed links, and canonical events. This is the durable substrate.
2. **Signal layer:** candidate gaps, candidate bridges, candidate tensions,
   proof readiness, observer rerankings, and review queues. These are derived
   projections over state, not new truth objects.
3. **Review loop:** CLI proposals, proof packets, quality tables, MCP/HTTP
   inspection, and benchmark reports. This is where humans and agents inspect
   state, accept or reject proposals, and write corrections back.

The natural act of using Vela should sharpen the frontier: compile can create
candidate state, check exposes weak state, review writes accepted corrections,
proof freezes a reviewable state, MCP makes agent use inspectable, and
benchmarks reveal drift. If a feature does not improve that loop, it does not
belong in v0.

## Current realization

The broader theory behind Vela is larger than the current release.

Vela v0 directly realizes the **state layer**:

- finding bundles
- evidence atoms
- condition records
- typed links
- canonical events
- replayable frontier snapshots

The release does **not** yet claim full realization of the later layers:

- **Runtime:** first-class experiment/protocol objects and writeback from live
  execution systems
- **Network:** federation, institutional propagation, and durable multi-party
  coordination

Constellations belong to that broader theory as the navigable projection of
frontier state. In v0, gaps, bridges, tensions, and review queues are early
state-derived surfaces, along with dependency and contradiction structure, not
a separate product promise.

## State first

The durable object is the frontier:

- finding bundles
- assertion fields inside findings
- evidence and provenance
- entities and conditions
- frontier epistemic confidence, extraction confidence, and confidence components
- typed links
- canonical review and correction events
- replayable state transitions
- signatures where available

The central primitive is the finding bundle. `assertion` is a field inside that
bundle, not a standalone public object. Source, evidence, observation,
condition, method, review event, confidence update, and checkpoint boundaries
must stay explicit so humans and AI can inspect where interpretation entered the
state.

`confidence.score` means bounded epistemic support for the finding as currently
represented in frontier state. It is not a truth label, not extraction
accuracy, and not a standalone review consensus measure. Review state changes
how the score should be interpreted through proposals, canonical events,
contestation flags, and proof freshness.

Vela may ingest agent traces, synthetic papers, experiment logs, notebook
entries, and benchmark outputs, but those are source artifacts. They should be
promoted into active frontier state only through evidence, conditions, review,
and canonical events.

A source record says where an artifact came from. An evidence atom says exactly
which span, row, measurement, run, metric, or weak provenance unit bears on a
finding. Vela should never let a citation silently stand in for evidence.
Condition records say where the claim stops: species, model system, method,
comparator status, exposure versus efficacy, and translation scope remain
visible so frontier state does not become a polished overgeneralization.

Correction propagation in v0 means replayable downstream review impact over
typed links, review state, and proof freshness. It is not automatic frontier
consensus.

The portable baseline is `frontier.json`, a materialized snapshot with an
embedded canonical event log. `vela check` validates event integrity and replay
status before a snapshot is treated as proof-ready. Git is the expected
versioning layer: branches, commits, diffs, and review history should remain
normal Git concepts.
Internal code may still contain older names, but public docs and examples should
say `frontier`.

## Conservative claims

Automated outputs are candidate surfaces:

- Candidate contradictions are leads for review, not resolved disputes.
- Candidate gaps are review leads over possible underexplored areas, not
  guaranteed experiment targets.
- Candidate bridges are possible cross-domain links, not discoveries.
- PubMed checks are rough prior-art signals, not novelty proof.
- Retraction propagation is simulated impact over declared dependency links.
- Observer policies are policy-weighted rerankings, not definitive stakeholder
  models.

Every summary should cite finding IDs where possible and preserve explicit
caveats.

## Calibration anchors

Benchmark fixtures are Vela's standard candles: reviewed calibration anchors
used to detect drift. They are not evidence that Vela is scientifically superior
to other systems. A benchmark item should be small, inspectable, tied to a
specific finding/entity/link/workflow expectation, and correctable by normal
frontier review.

## Failure modes

Vela should name its failure modes rather than hide them:

- **Evidence lock-in:** early extractions become trusted because they appeared
  first.
- **Graph conservatism:** declared links over-amplify connected findings and
  miss off-graph evidence.
- **Small-review fragility:** a few corrections make the frontier look more
  certain than it is.
- **Objective drift:** truth, relevance, commercial interest, and actionability
  collapse into one score.

Signals, review queues, proof packets, and benchmark gates exist to expose
these problems early.

## Flagship proof

The proof path is the BBB/Alzheimer frontier. It is a protocol example for
state, review, replay, and proof mechanics, not an authoritative Alzheimer's BBB
science map.

The path is:

1. Open the bounded frontier, or compile candidate state only as onboarding.
2. Check the frontier state.
3. Record at least one review, caveat, correction, or rejection when the state is
   weak or overbroad.
4. Inspect key findings, evidence, provenance, conditions, and confidence.
5. Compare candidate tensions without flattening disagreement.
6. Export a proof packet with scope notes, event logs, and replay artifacts.
7. Serve the same frontier state over MCP/HTTP.

The BBB proof must stay bounded. It can show better working memory, accepted
state transitions, replay, and proof invalidation. It must not claim complete
field coverage, validated novelty, clinical truth, trusted automated extraction,
or automated research judgment.

## Out of scope for v0

Do not present the current release as:

- a Hub or desktop product
- a lab/runtime environment
- an autonomous agent loop
- a replacement for autonomous scientist systems
- a federation or exchange network
- GitHub for science
- a full science operating system
- an institutional review platform

Those ideas may exist as roadmap or thesis work, but they are not current public
release claims.

## Later work

Future work can extend the kernel only after the bounded frontier primitive stays
credible under real review:

- signed and identity-backed review workflows beyond the current optional
  signature support
- richer frontier diff, merge, and comparison workflows
- first-class contradiction records, credal uncertainty updates, propagation
  obligations, and constellation projections once their replay/proof/review
  semantics are clear
- source coverage for agent traces, experiment logs, benchmark outputs, and
  notebooks
- first-class protocol, experiment, result, hypothesis, and observation objects
  once they have replay, writeback, proof/export, and review/merge semantics
- dedicated constellation and visualization interfaces
- network propagation, institutional sharing, and broader federation

These are roadmap items, not v0 release claims.
