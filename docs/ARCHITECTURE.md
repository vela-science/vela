# Architecture

Vela's mental model in one document. Where the substrate sits in
the science stack, why the position is structurally available, and
what becomes possible above it.

The complementary documents are `CORE_DOCTRINE.md` (the rules the
substrate follows), `PROTOCOL.md` (the v0 language kernel), and
`THEORY.md` (the math and the theory-side nomenclature). This file
is the strategic frame they all sit inside.

## 1. The unit problem

Existing scientific infrastructure is silo'd because every project
models the wrong unit. Each silo is correct about its own piece and
silent about how the pieces fit together.

| System | Native unit | What it owns | What it can't see |
|---|---|---|---|
| Crossref | DOI | Citation graph | Datasets, models, replications, code |
| ORCID | Researcher | Identity | What the researcher actually believed, when |
| bioRxiv, arXiv | Paper PDF | Preprints | Anything inside the paper |
| Zenodo | File deposit | Generic blobs + DOI | What the deposit means |
| HuggingFace | Model weights | ML artifacts | Which experiments produced them |
| W&B, MLflow | Experiment run | Training metrics | What claims the runs support |
| PDB, UniProt | Domain artifact | Atomic coords, sequences | What hypothesis the artifact tests |

Notice what is missing from every row. The unit "X is true under
conditions Y, supported by evidence Z, asserted by W, replicated in
V, with implications U" never appears. Each silo manages its piece
of evidence as if it were the whole picture, and the picture never
composes.

This is not because nobody noticed. Modeling claims as first-class
is structurally hard. It needs content-addressing, signed events,
federation, replication-as-object, multi-actor identity with
calibration, and evidence chains that span domains. Each of those
alone is a serious engineering project. Together they are an order
of magnitude harder than tracking artifacts.

The structural opening for Vela is that nobody has done that work.

## 2. The architecture

The science stack has six layers. Vela occupies one of them.

```
L4  applications      — what becomes possible above the substrate
                        (consensus dashboards, calibration leaderboards,
                         replication markets, retraction cascades, …)

L3  surfaces          — agents, IDEs, renderers
                        (Phylo, Atlas, Future House, The Stacks,
                         Vela CLI + Workbench)

L2  Vela              — the claim ledger
                        state · runtime · network

L1  evidence          — existing silos, wrapped not replaced
                        (Zenodo, PDB, HuggingFace, GitHub, ORCID,
                         Crossref, W&B, MLflow, …)

L0  compute           — GPUs, foundation models, domain models
```

The arrows in this stack are what matter:

- Evidence flows up into Vela. A `vd_` references a Zenodo deposit.
  A `vc_` references a Git commit. An `actor.id` resolves through
  ORCID. The substrate never reimplements what L1 already does well.
- Surfaces flow down into Vela. Phylo's agents propose claim
  mutations. The Stacks renders Vela findings to human readers.
  The CLI is the substrate's first-party surface.
- Applications query down into Vela. A consensus dashboard reads
  the canonical state. A calibration leaderboard walks the
  predictions and resolutions. The applications never reimplement
  the substrate; they interrogate it.

L2 is the spine. Everything else either feeds it (L0, L1), uses
it (L3), or queries it (L4).

## 3. What Vela actually contains

The kernel objects are the spine.

**State** (content-addressed, immutable):

- `vf_<hash>` — finding, the atomic claim
- `vfr_<hash>` — frontier, a bounded scope of claims
- `vrep_<hash>` — replication attempt
- `vd_<hash>` — dataset reference
- `vc_<hash>` — code artifact
- `vpred_<hash>` — prediction
- `vres_<hash>` — resolution
- `vev_<hash>` — signed canonical event
- `vpr_<hash>` — proposal (a draft mutation)
- `actor` — registered identity with calibration record
- `peer` — federated hub registration
- `signature` — Ed25519, single or k-of-n threshold

**Runtime** (deterministic queries over state):

- `compute_confidence` reads replications, evidence type, sample
  size, conditions, contested flag, and (v0.38.1) causal grade.
- `consensus_for` aggregates claim-similar findings with optional
  causal filter and weighting scheme.
- `audit_frontier` produces per-finding identifiability verdict.
- `propagate_correction` cascades retraction, replication, and
  reinterpretation through the link graph.
- `calibration_records` derives Brier, log score, hit rate per
  actor over resolved predictions.
- `replay_report` reconstructs frontier state from the event log.

**Network** (federation):

- Hub publishes signed registry entries pointing at network locators.
- Peer registry per frontier; sync detects nine kinds of conflict.
- Cross-frontier links resolve `vf_<id>@vfr_<id>` references at
  query time.
- No single hub controls the network.

Every kernel object can carry a stable pointer into an L1 silo:
`vd_X.url` is the Zenodo URL, `vc_Y.repo + commit` is GitHub,
`actor.id` is shaped to accept ORCID. The substrate is what wraps
the silos into a single signed manifest.

## 4. The three rules and three properties

Three rules define the substrate.

1. **Every claim has a stable identity.** Content-addressing.
2. **Every state change is signed and append-only.** Cryptographic
   ledger.
3. **The substrate works across organizations.** Federation.

Three properties fall out of those rules.

- **Verifiability.** Anyone can check the history hasn't been
  tampered with. The signature chain is the audit trail.
- **Composability.** Claims reference other claims, datasets, code,
  replications by stable ID. The substrate is self-similar across
  domains.
- **Replayability.** The field's belief state can be reconstructed
  at any historical point. Time travel is a query.

These three properties are what L4 applications need. None of the
existing silos provides all three.

## 5. What becomes possible

Once L2 exists, things that are currently impossible or expensive
become cheap. The pattern: anything that requires reasoning over
claims becomes a query against the substrate.

### Real-time field consensus

"What does the field hold about TREM2 R47H risk?" Computed
deterministically from signed claims, weighted by replication count
or reviewer trust, with credible interval. Today this requires
reading three review papers and asking a colleague. With Vela, it
is `vela consensus vf_X --weighting replication`.

### Calibration leaderboards

Every researcher accumulates a public, reproducible track record of
how often their predictions matched reality. Brier score per actor,
per topic, time-windowed. A forecaster's GitHub contribution graph.
Today this does not exist. With Vela, it is the v0.34/v0.40.1
calibration runtime.

### Replication bounty markets

A finding gets posted; anyone with the wet-lab capacity claims a
bounty by signing a `vrep_` against it. Failed replications subtract
from the original's confidence; successful replications in different
conditions raise it. Today replication is unfunded thankless work.
With Vela, replication is a first-class economic event.

### Retraction cascade detection across organizations

A paper retracts on hub A. Hub B and C, federated peers, propagate
the cascade to every downstream finding within seconds. The
substrate flags retracted citations before they hit publish. Today,
retracted papers get cited for five-plus years. With Vela, the
cascade is real-time.

### Causal-typed reasoning surfaces

"Show me intervention-grade evidence only for amyloid-beta clearance
via TfR antibodies." Today this is forty hours of literature
curation. With Vela, it is `vela consensus --causal-claim
intervention --causal-grade-min rct`.

### Time-traveling literature reviews

"What did the field believe about lecanemab in March 2024?" Replay
the event log to that timestamp and rerun consensus. Today this is
literally impossible. With Vela, it is a query flag.

### Cross-frontier composition

"If amyloid drives tau drives cognitive decline, what does the
BBB-delivery frontier imply about cognitive efficacy of
amyloid-targeting antibodies?" Bridge inference across linked
frontiers. Today this is PhD-thesis territory. With Vela, it is
cross-frontier link resolution plus the bridge runtime.

### Auditable AI-for-science

Every claim a Phylo, Atlas, or Future House agent extracts is signed
by the agent, reviewable by humans, traceable to evidence. The AI
explosion produces a verifiable claim torrent instead of an
unverifiable one. Today, agent output is unauditable. With Vela,
every claim carries a signed provenance chain.

### Negative-results substrate

Every prediction that did not resolve correctly, every finding that
failed replication, every claim that retracted, all queryable, all
weighted into consensus. The negative-results problem solved
structurally rather than culturally. Today, failure data disappears.
With Vela, failure data is first-class.

### Verifiable open peer review

Reviewers sign their critiques as `finding.reviewed` events. The
reviewer's calibration record (how often did their accept/reject
calls match the field's eventual verdict?) accumulates over time.
Today, peer review is anonymous with no track record. With Vela,
reviews are signed and reviewer calibration is queryable.

### Trust graphs

Lab A trusts lab B's signed reviews in neuroscience but not in
chemistry. The substrate carries this as edges in the actor graph.
Federation honors the trust topology. Today, trust is implicit and
ephemeral. With Vela, trust is queryable.

### Provenance for downstream products

A drug-discovery startup's pipeline traces back to the specific
`vf_` claims it rests on, with full evidence chains and replication
status. Pharma due diligence becomes 100x cheaper. Regulatory review
becomes auditable. Today, pipeline-to-paper traceability is months
of manual work. With Vela, it is a query.

The list is long because there is a lot enabled by getting the unit
right. The common pattern is that anything that requires reasoning
over claims becomes a substrate query.

## 6. Strategic position

The position to occupy is L2 narrowly and well. Every other layer
has well-funded incumbents. None of them is competing at L2 because
none of them has tried to model claims as the unit. That is the
structural opening.

Five operating principles follow from the position.

1. **Stop competing with L3.** Do not build a Vela agent runtime to
   compete with Phylo. Do not build a Vela publishing layer to
   compete with The Stacks. Accept their output, render to their
   format. The agents and the publishers are customers of the
   spine, not rivals to it.

2. **Aggressively integrate with L1.** Every silo should wrap as
   evidence. Build adapters: ORCID for actor identity, Crossref for
   paper provenance, Zenodo for `vd_` deposits, GitHub for `vc_`
   artifacts, HuggingFace for model checkpoints. Each adapter
   expands the surface area of what claims can be made about.

3. **Make L4 applications the proof points.** Field-consensus
   dashboards, calibration leaderboards, replication bounty
   markets. The visible artifacts that let people see what the
   substrate enables. The BBB causal audit page is the prototype;
   the calibration leaderboard is next; the replication market is
   third.

4. **Position Vela as the connector.** Phylo's, Atlas's, and Future
   House's outputs do not connect to each other. Vela is the
   connector. That is a sellable pitch to each of them as a partner
   rather than a competitor.

5. **The Linux moment is when one major effort decides to publish
   their generated claims through Vela for verifiability and
   composability.** Not adoption by 100 labs. Adoption by one
   well-known one whose adoption others follow.

## 7. The summary

Vela is the L2 of science. The atomic unit is the claim. The
substrate properties are verifiability, composability, and
replayability. The substrate's edge is that it owns the claim
layer, which nobody else has tried to own. The strategic move is
to occupy L2 narrowly, integrate with L1 silos as evidence sources,
let L3 surfaces use it, and produce L4 applications as proof
points.

Everything in the codebase is craft applied to this model. The
model itself is the bet.

## 8. What this implies for the roadmap

The substrate work is essentially done at v0.42. The kernel objects
are first-class, the runtime is comprehensive, the network has its
first real federated peer, and the daily-driver CLI is shaped like
Git. The next phase of work is not more substrate. It is closing
the gap between substrate and adoption.

In rough order of leverage:

- One L1 adapter that compounds. ORCID for actor identity is the
  smallest, most universally useful. Each Vela actor.id can resolve
  through the ORCID directory. After that, Crossref for paper
  provenance and Zenodo for dataset deposits.

- One L4 application that demonstrates the substrate's value beyond
  what Phylo or The Stacks can do. The replication cascade is the
  best candidate: a failed replication lands, the substrate visibly
  drops downstream confidence across N dependent findings, and the
  audit page surfaces what is now in question. Phylo cannot show
  this because it does not have replication-as-object. The Stacks
  cannot show this because it does not track downstream
  dependencies.

- Reference book. A narrative document that walks a working scientist
  through their first frontier, first replication, first federation,
  first audit. The Pro Git of Vela. Two evenings of work, but it is
  the document every adoption attempt routes through.

- A second-language implementation. A Python kernel that reads and
  writes the same `.vela/` layout proves the protocol is real
  rather than "Will's Rust project." Not a full reimplementation,
  just the validator, canonical-bytes derivation, and a handful of
  operations. Probably two or three weeks of careful work.

- Concrete outreach to five to ten researchers whose work would
  benefit immediately. Personal contact, offer to bootstrap their
  first frontier, walk them through the daily-driver CLI. The
  substrate is sufficient; the question is whether anyone outside
  the reference implementation is going to use it.

These are adoption moves, not kernel moves. They are the work that
turns a beautifully architected solo project into a substrate the
field reaches for.
