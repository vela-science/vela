# ADR 0004: Falsify the need for a scientific dependency primitive

- Status: Accepted 2026-07-15. On 2026-07-15 the project owner authorized
  Phase 0 and experiment-only Phase 1 scaffolding to begin in parallel with
  ADR 0003. The blind handoff, ADR 0004
  same-information agent benchmark, API spend, human ceremony, and
  outside-evidence claims remain closed until their own gates are satisfied.
  Phase 0 is now frozen against released Vela `v0.800.12`. The first Phase 1
  resolver candidate is incomplete by design: it rejects structural
  mismatches but returns
  `unresolvable:authority_snapshot_porcelain_missing` for a matching internal
  fixture. That is a gap result, not evidence that current objects suffice.
  Released `v0.800.13` at commit
  `b3076f8935a38ecaef252e7f062648794cc7cd07` carries that negative result;
  public conformance run
  [29455939576](https://github.com/vela-science/vela/actions/runs/29455939576)
  and immutable release run
  [29456349900](https://github.com/vela-science/vela/actions/runs/29456349900)
  succeeded at the exact release commit. Campaign commit
  `7d26b2b050aa1dbcfa7864d184c45250c3c21f26` pins it.
  Phase 1A completed as an internal fixture. Its registered ablation
  found one evidence-availability gap: the signed event binds a DecisionPlan
  root, but released decisions do not retain the corresponding canonical
  preimage. Released `v0.800.14` at commit
  `54d1ab7810f0f74ca59ba0bedffed598c3d6924e` contains the narrow retention
  fix from exact implementation commit
  `e083ffd05ba80b3e9ad06c4fa3b377a2e5ac75dc` and semantic-event signature
  hardening. Public conformance run
  [29470053318](https://github.com/vela-science/vela/actions/runs/29470053318)
  and immutable release run
  [29470054208](https://github.com/vela-science/vela/actions/runs/29470054208)
  succeeded at that exact release commit. This is released engineering
  evidence, not outside-use evidence or a dependency verdict. Follow-up
  `v0.800.15` repairs task-first work-snapshot replay parity; it does not change
  this retained-evidence seam or supply new dependency evidence.
- Scope: exact cross-frontier composition, later-root correction handling, and
  the smallest public contract that independent producers and consumers might
  need.
- Relation to ADR 0003: this ADR does not expand or replace the active
  task-first workflow implementation. It is the registered research lane in
  the current ADR 0003/ADR 0004/Canopus program.
- Authority: this document is an accepted engineering decision. It is not a
  signed scientific event, does not exercise a human key, and does not replace
  ADR 0003's authority boundary.

## Decision summary

Adopt the research question, not the memo's proposed replacement architecture.

Vela will first try to express one exact scientific dependency using its
current narrow waist:

- Git commit and tree identity;
- Receipt v1 and its full canonical root;
- the existing proposal-to-authority boundary;
- the finding's readable `vf_` handle paired with its full
  `finding_hash()`;
- verifier attachments paired with full canonical byte roots;
- the exact signed authority event and the full event-log root; and
- deterministic replay at a delivered Git root.

The experiment may carry an exact dependency observation inside Receipt v1's
already content-bound extension space. That observation is experimental input
to an independent reader, not a new accepted wire object.

Vela will add a public dependency primitive only if a blind handoff proves that
the current objects cannot represent or replay a necessary invariant. If a new
invariant is required, the prelaunch protocol will hard-cut to the smallest
form that supplies it. Vela will not ship a parallel `handoff.v1` stack,
seven new visible object families, mandatory TUF metadata, or three new CLI
commands in anticipation of that result.

The experiment must allow a standards-based Git profile to beat Vela. If Git,
DSSE or in-toto, exact locks, and a small deterministic status rule provide the
same safety with less special machinery, Vela should become the interoperable
profile and tooling around those standards rather than claim a new protocol
category.

The registered Phase 1A result justifies one smaller engineering change. New
decisions should retain the exact canonical DecisionPlan preimage at
`records/decision-evidence/decision-root/<decision-root-hex>.json` in the same
recoverable transaction as the signed decision. The signed event binds that
preimage through its domain-separated decision root. Retention preserves
inspectable evidence; it adds no event, authority rule, dependency object, or
status reducer. Removing the file leaves reducer replay, event-log identity,
and event signatures unchanged.

## Finite implementation ledger

This ledger replaces a growing list of open-ended research questions. A result
may change one row only with registered evidence; it may not silently widen the
experiment or promote a primitive by analogy.

| Item | Status | Exact boundary |
| --- | --- | --- |
| Frozen Phase 0 registration and negative resolver result | **PROVED** | released result is `unresolvable:authority_snapshot_porcelain_missing`; it promotes no object |
| Public release and campaign pin for the negative experiment | **PROVED** | `v0.800.13`, exact public runs, and parent commit recorded above |
| Phase 1A canonical-custody registration and hostile vectors | **PROVED — INTERNAL** | registered `v0.800.13` run passes 14 custody vectors with zero false verified outcomes; this is fixture evidence only |
| Canonical exact-checkout reader | **PROVED — INTERNAL** | exact Git/tree/replay/proof/view/lock roots agree for the ordinary Vela-produced fixture under bounded offline execution |
| Read-only named-decision inspector | **PROVED — LOCAL** | 29 registered Rust/Python classifications agree, including invalid applied-event signatures, and 10 focused Rust tests pass; the pure API has no path, write, socket, clock, registry, or key parameter |
| DecisionPlan-preimage ablation | **PROVED — INTERNAL** | root-only returns `unresolvable:decision_preimage_unavailable`; retained preimage returns `verified:decision_evidence_bound`; event and replay roots remain equal |
| Root-keyed DecisionPlan evidence retention | **PROVED — LOCAL** | exact implementation commit `e083ffd05ba80b3e9ad06c4fa3b377a2e5ac75dc` writes the canonical preimage as removable `CanonicalEvidence`, verifies every matched semantic-event signature, and proves deletion leaves replay and signed events unchanged |
| Public release of the retained-preimage seam | **PROVED — RELEASE** | `v0.800.14` is tagged at `54d1ab7810f0f74ca59ba0bedffed598c3d6924e`; public conformance and immutable release runs `29470053318` and `29470054208` succeeded at that exact commit |
| Git/DSSE/in-toto same-information baseline | **IMPLEMENTING** | the simpler standards profile is allowed to win at equal semantics |
| First-party resolver, CI, and context projections | **IMPLEMENTING** | all consume one exact tuple, remain removable, and add no authority object |
| Canopus paired agent benchmark | **IMPLEMENTING** | **Vela Research Harness** / `vela-research-harness`, CLI `canopus`; preregistered equal-budget runs only |
| Independent substantive safety handoff and reader agreement | **OPEN — INDEPENDENT** | outside A, B, and Reader C complete the exact-root handoff without maintainer repair |
| Real authority decision and later correction | **OPEN — HUMAN** | the relevant key holder decides through existing Vela ceremony and later authority state is delivered |
| Independent complementor using the narrow contract | **OPEN — INDEPENDENT** | a third-party application removes real work without schema or authority proliferation |
| Claim that Vela is a useful new foundation | **OPEN — INDEPENDENT** | held-out causal lift, producer/verifier substitution, correction advantage, and three applications all pass |
| Hosted parent execution evidence after the complete release input | **OPEN — INFRASTRUCTURE** | a zero-step billing-blocked run is not evidence for or against the experiment |
| Any dependency object, event kind, command family, status reducer, or primitive beyond the registered retained-preimage seam | **REJECTED** | Phase 1A justified evidence retention only; no broader protocol expansion follows from it |
| A graph, wiki, cache, Canopus database, or hosted service as authority | **REJECTED** | every projection remains derived and deletable |
| Short handles, mutable branches, prose reconstruction, automatic truth propagation, or central-registry dependence | **REJECTED** | every accepted dependency judgment remains full-root, authority-scoped, and replayable |
| The synthetic derived-view fixture as canonical-custody evidence | **SUPERSEDED** | Phase 1A must use ordinary Vela-produced canonical state |
| The earlier broad proposal for a parallel dependency stack | **SUPERSEDED** | the experiment may promote only the smallest evidence-justified read-only invariant |

Canopus in this ledger names only the reactivated orchestration product above.
Earlier Canopus protocol, stewardship, and `canopus-lean` designs remain
historical and provide no evidence for this ADR.

## Mission

> Make a scientific result safely reusable as a versioned dependency whose
> evidence, verifier, authority, and corrections are independently checkable
> from Git.

A more product-facing name for the capability is **verifiable scientific
composition**. **Corrigible scientific dependencies** names its decisive
safety property: a consumer can identify exactly what it relied on and can
reconsider that reliance when a later delivered authority state changes.

## The exact problem

Given immutable artifacts and an untrusted sequence of delivered Git roots,
can an independent consumer:

1. identify an exact claim revision;
2. reproduce or inspect the named verification;
3. verify which scoped authority admitted that revision;
4. bind a substantive child result to the exact premise it consumed; and
5. deterministically classify that dependency after a later authorized
   correction, supersession, withdrawal, or fork,

without trusting the producer, sharing a database, using a hosted Vela service,
reconstructing the claim from prose, or asking a Vela maintainer to repair the
handoff?

This is not a global-truth or consensus problem. Authorities and frontiers may
legitimately disagree. The required guarantee is exact, authority-scoped, and
relative to one selected lineage.

## A safe handoff is necessary, not sufficient

The blind handoff decides whether Vela has a coherent interoperability and
correction primitive. It does not by itself show that the primitive is useful
enough to become infrastructure.

ADR 0004 therefore separates two claims:

1. **Safety claim:** independent systems can compose and later reconsider an
   exact authority-scoped result without hidden trust or maintainer repair.
2. **Usefulness claim:** the resulting state makes later scientific work
   measurably easier, safer, or more productive than the same producer given
   ordinary files, a verifier, or a matched flat packet containing the same
   facts.

Vela earns the word **primitive** only if it demonstrates an operation that is
both irreducible and generative:

- **irreducible operation:** carry a semantically closed research obligation
  and accepted judgment across producers, verifiers, corrections, and
  downstream derivations so later work inherits exactly what remains usable;
- **causal leverage:** under a matched budget, inherited Vela state improves a
  later producer's verified progress, correction time, or error rate;
- **producer substitution:** state created by one model, solver, or person
  remains useful to a materially different producer;
- **verifier substitution:** the same narrow contract works across Lean, an
  exact native checker, and a proof-logging solver without a new authority
  model;
- **local value:** one repository benefits before a network, registry, or
  hosted service exists;
- **complementor value:** independent tools can build useful read-only
  applications over the same objects without entering the trust path; and
- **necessity:** simpler same-information ablations fail on at least one
  important capability rather than merely offering a less polished CLI.

Git, Unix and Linux, SQLite, OCI, OpenTelemetry, Stripe's intent model, and
Lean's kernel/library ecosystem are design analogies, not empirical evidence
that Vela's architecture is correct. Their relevant common pattern is a small,
locally useful contract that is explicit about failure and stable enough for
independent implementations and applications. The ecosystem, not object
count, supplies the breadth.

## Why this question is timely

Several trends increase the value of a neutral state layer while also making a
weak one easy to commoditize:

- scientific workbenches are capturing private runs, tools, artifacts,
  histories, and reviewer actions;
- model and provider choice is volatile, making portable accepted state more
  durable than one workbench's memory;
- agent output is growing faster than qualified human review capacity;
- exact and cheap verifiers are spreading through formal mathematics,
  certificate-producing solvers, simulations, data systems, and programmable
  experiments;
- scientific objects are decomposing below papers into claims, artifacts,
  proofs, workflows, datasets, and typed checks;
- persistent agents need bounded, active context rather than unscoped retrieval;
  and
- corrections, verifier changes, and stale evidence become more dangerous when
  thousands of machines can reuse a result immediately.

Basic provenance, signatures, graphs, and agent memory will be features of many
workbenches. Vela is clearly useful only if it supplies the portable scientific
transition those systems do not: exact evidence, explicit verification,
scoped authority, dependency standing, and correction, all checkable from
ordinary Git.

## Adjacent systems: scaffold, do not replace

ADR 0004 treats the emerging scientific stack as inputs and complementors. It
does not ask Vela to become an experiment runner, paper server, general
provenance ontology, workflow language, or knowledge-graph database.

| Adjacent system | What it owns | Vela boundary |
| --- | --- | --- |
| [Hypothesis Evolution Protocol](https://arxiv.org/abs/2607.09195) | An append-only, hash-chained event registry for hypotheses, evidence, elicited belief, lineage, and lifecycle activity. Its reported v1 study is single-agent, evidence validity is agent-self-certified, and code is promised on publication. | Import a run as producer provenance. Content-bind its exact log and artifacts, but never convert self-assigned belief or lifecycle state into accepted standing. |
| [OpenResearch](https://openresearch.sh/docs) and [Claude Science](https://www.anthropic.com/news/claude-science-ai-workbench) | Worktrees, compute environments, commands, experiment history, outputs, and workbench interaction. | Bind the exact Git root, command, environment, outputs, and verifier result in a Receipt. An error-free run is not scientific reproduction, authority, partnership, or adoption evidence. |
| [Diderot](https://projectdiderot.com/about) and the [Leiden Declaration on AI and Mathematics](https://leidendeclaration.ai/) | Experimental publication, authorship, disclosure, credit, and human-responsibility norms. Diderot permits AI authors while reserving certificates to humans; Leiden emphasizes disclosure, independent verification, human responsibility, and statement fidelity. | Preserve disclosure and scoped authority policy rather than encode one universal authorship rule. Diderot is an interface hypothesis, not a partner, verifier, or adoption signal. |
| [CodeGraph](https://github.com/colbymchenry/codegraph#how-it-works), [Graphify](https://github.com/Graphify-Labs/graphify), and [LLM-Wiki](https://arxiv.org/abs/2605.25480) | Rebuildable code/research graphs, extracted versus inferred links, linked explanations, and error memory. | Pin each projection to exact source roots and label inference and staleness. Graphs and wikis remain disposable context porcelain, never authority-bearing state. |
| [CWL](https://www.commonwl.org/v1.2/Workflow.html), [RO-Crate](https://www.researchobject.org/ro-crate/1.1/), and [W3C PROV](https://www.w3.org/TR/prov-dm/) | Portable workflow description, research-object packaging, and general provenance vocabulary. | Import and content-bind their artifacts. Do not recreate workflow execution, packaging, or generic provenance inside the Vela protocol. |
| Lean and shared formal libraries | Exact kernel checking and a large reusable formal corpus. | Retain statement-fidelity review, scoped admission, external evidence, and cross-verifier handoff as separate concerns. |

One foreign-workbench import test should therefore accompany the blind case:
consume an exported Git/log bundle from a workbench or HEP-style run, bind its
environment and output roots, and prove that producer-side belief, ranking, or
success labels remain provenance until an independent verifier and authorized
decision say otherwise. This is an adapter test over existing objects, not a
new public primitive.

## Evidence ladder for a new foundation

ADR 0004 uses a ladder so byte correctness, product polish, scientific output,
and ecosystem proof cannot be conflated:

| Rung | Evidence | What may be claimed |
| --- | --- | --- |
| 0. Semantic closure | Every truth-relevant mutation changes a full root or fails replay | The profile is integrity-complete for its declared scope. |
| 1. Cold handoff | Independent A, H, B, and C complete exact offline composition | The profile is interoperable in one controlled case. |
| 2. Useful composition | B creates a substantive verified child and consumes the exact parent | Vela supports real cross-producer reuse, not metadata exchange. |
| 3. Operational correction | A later root identifies the right review set faster and more accurately than manual reconstruction | Corrigibility has operational value. |
| 4. Causal compounding | Vela state beats same-information flat and retrieval baselines on held-out work | State structure increases later productivity. |
| 5. Generality | The effect survives producer and verifier substitution in multiple families | The capability belongs to the substrate rather than one runtime. |
| 6. Ecosystem extension | Independent complementors build useful removable applications over the same waist | The primitive unlocks applications without protocol growth. |
| 7. Outside recurrence | Independent teams repeat the loop on work they chose | Vela has early infrastructure adoption evidence. |

No lower rung implies a higher one. In particular, internal conformance vectors
do not prove interoperability, a verified child does not prove scientific
novelty, and one external loop does not prove a new foundational category.

## What the current system already supplies

| Need | Existing Vela or Git construct | Boundary |
| --- | --- | --- |
| Immutable transport | Git blobs, trees, commits, bundles, ancestry, and ordinary remotes | Git says nothing about scientific standing. |
| Producer evidence | Receipt v1 plus its full `sha256:<64>` canonical root | A receipt is neither verification nor acceptance. |
| Logical and revision identity | `vf_` finding handle plus full `finding_hash()` | The short handle is readable routing, never sufficient security identity. |
| Separate verification | Verifier attachments, claim digest, method, outcome, and retained bytes | The `vva_` handle must likewise be paired with full canonical bytes or a full digest. |
| Offered versus authorized change | Public proposal semantics followed by an authority-bearing event | Proposal-store layout is not a read contract, but proposal semantics remain part of the write/authority boundary. |
| Scoped decision | Signed canonical event, authority identity, and parent state binding | Event handles are short. The experiment must retain the full event-content root and signature. |
| Frontier state | Full event-log and snapshot roots plus deterministic replay | A root proves its own state, not the existence of a future correction. |
| Derived relationships | Canonical stored link `depends`, projected as graph edge `depends_on` | Finding links are mutable review surfaces excluded from `finding_hash()`; they are not dependency locks. |

This inventory rejects three unnecessary reinventions:

1. Findings already pair a logical `vf_` handle with an exact revision hash;
   no new global claim ID is assumed.
2. Proposals remain the non-authoritative side of the trust boundary even if
   an accepted-state reader does not consume proposal storage.
3. Existing Git ancestry, causal event roots, and signed decisions must be
   tested before importing TUF's extra roles, expiry, timestamp, and freshness
   machinery.

## The precise suspected gap

Current Vela has no clearly normative, content-bound statement that says:

> This child consumed this exact parent revision, admitted by this exact scoped
> authority decision, with these exact verifier and evidence roots, for this
> exact premise role.

`FindingBundle.links` cannot fill that role. They are intentionally excluded
from `finding_hash()`, may be appended without a state event, and exist as
review and graph surfaces. The stored canonical relationship name is
`depends`; `depends_on` is a typed projection. Neither spelling turns the
link into an immutable authority-bearing dependency pin.

Receipt v1 can content-bind an experimental dependency observation in its open
extension space. What it does not currently define is the observation's
normative meaning, later-root continuity rule, or status transition. The
experiment must determine whether those semantics require:

- only a small profile and derived reader;
- one new content-bound field on an existing object; or
- a genuinely new protocol object.

The burden of proof increases down that list.

Phase 1A closed two narrower implementation questions without resolving this
composition question. Exact checkout showed that canonical custody
exists in Git plus Vela's replay and proof surfaces. The registered ablation
showed that retaining a bound DecisionPlan preimage makes one named
decision inspectable. Historical decisions and `v0.800.13` still expose only
the root, and the current pure inspector handles one decision with one answer.
No public resolver yet discovers the retained path or assigns dependency
standing.

## Experimental dependency observation

The blind run will use this logical tuple. It is a test profile, not a public
schema commitment:

```text
DependencyObservation {
  schema
  parent_frontier_id
  parent_git_commit
  parent_git_tree
  parent_event_log_root
  parent_snapshot_root

  finding_id                 # readable vf_ handle
  finding_revision_root      # full finding_hash()

  decision_event_id          # readable vev_ handle
  decision_event_content_root
  decision_signature
  authority_id

  receipt_roots[]            # full sha256 roots
  verifier_attachments[] {
    attachment_id            # readable vva_ handle
    attachment_content_root  # full canonical byte root
  }

  premise_digest
  role                       # hard | soft | data | method | contextual
}
```

Every security-relevant comparison uses full roots. Short handles may select a
candidate object, but the reader must reject a missing or mismatched full root.

For the first run, Producer B may place this tuple at
`environment["vela:experimental_dependencies"]` in its Receipt v1. The
whole-receipt binding then commits to the tuple without changing Receipt v1.
The ordinary landing path must continue to treat the extension as producer
provenance; only the experiment reader interprets it. If that placement proves
semantically wrong, that finding is evidence for the smallest future change.

## Continuity and correction

An immutable Git root cannot reveal an event that has not happened. The
experiment therefore uses an explicit later-root delivery model:

1. Producer B receives and pins parent root `C0`.
2. At `C0`, B verifies the exact finding revision, verifier material, scoped
   decision, and full roots before building the child.
3. A later root `C1` is delivered through any untrusted channel.
4. The reader proves or rejects Git ancestry and Vela event-history
   continuity from `C0` to `C1`.
5. It verifies the later authority event and deterministically recomputes the
   parent's standing.
6. It maps that standing and the declared premise role to a dependency status.

The experiment must distinguish:

- `satisfied`: the exact parent revision retains the required standing;
- `warning`: new information is relevant but the declared role remains
  usable under the profile;
- `review_required`: authority standing changed and a human or child
  frontier must reconsider reliance;
- `blocked`: the profile says the dependency may no longer be consumed;
- `stale`: the reader was given an older root than its recorded last-seen
  root;
- `forked`: the new root is valid but not a descendant of the selected
  lineage; and
- `unresolvable`: required bytes, roots, signatures, or verifier material are
  unavailable or invalid.

A parent correction changes the **standing of the dependency**. It does not
automatically prove the child false, rewrite the child receipt, or silently
mint a child-frontier authority event. Current
`finding.dependency_invalidated` fixtures demonstrate replay of an explicit
event; they do not establish automatic cross-frontier propagation.

The first continuity baseline is a portable last-seen full root plus verified
Git/event descendant relation. TUF remains an adversarial comparison and an
optional baseline variant. It becomes required only if the experiment exposes
rollback, freeze, delegation, or key-rotation requirements the simpler rule
cannot satisfy.

## Blind experiment

### Scientific case

Use finite graph theory with SAT/LRAT certificates and independently checked
transformation logic.

Producer A supplies a canonical graph `G` and evidence that it is
triangle-free with chromatic number four:

- canonical graph bytes;
- a four-coloring witness;
- a SAT encoding of three-colorability;
- an LRAT certificate that the encoding is unsatisfiable; and
- an independently runnable checker profile.

Producer B applies the Mycielski construction and establishes a substantive
child result for `M(G)`, such as triangle-freeness and chromatic number five.
The child must consume A's exact accepted revision as a hard premise. This
crosses verifier boundaries and creates a real new result without making
scientific novelty a protocol acceptance condition.

B's checker must consume the exact parent graph or certificate and the
transformation evidence. Removing, mutating, or substituting the parent must
fail or return `unresolvable`. Independently re-solving `M(G)` is useful
robustness evidence, but it does not count as dependency composition.

If this case is too large for the first cold run, use a smaller pre-registered
graph with the same A-to-B dependency shape. Do not substitute a metadata-only
citation or a pure same-repository Lean import.

### Independence roles

| Role | Requirement |
| --- | --- |
| Producer A | No Vela maintainer; own repository and domain tools; no hand-authored protocol JSON. |
| Verifier V1 | Clean-clone reproduction of A at the exact root. |
| Verifier V2 | Independent graph/certificate implementation; no Vela reducer or A checker reuse. |
| Human steward H | Reviews the exact Decision Brief and alone controls the decision key. |
| Producer B | Receives frozen instructions and exact roots; no contact with A or maintainers during the run. |
| Reader C | Independent implementation from the written profile; no Vela source reuse. |
| Red team R | Supplies stale roots, forks, mutations, missing bytes, and the correction drill; has no authority key. |
| Baseline team | Runs the same scientific task and authority rules with the standards profile. |

The protocol team freezes instructions, binaries, vectors, and allowed support
before the run. Every intervention is recorded. Installation help already
covered by public instructions may be given; artifact editing, semantic
interpretation, and bespoke repair are failures.

### Standards comparison

The first baseline is deliberately small:

- ordinary Git objects, commits, bundles, and ancestry;
- one DSSE-wrapped canonical scientific statement;
- a content-bound `science.lock` carrying the exact dependency tuple; and
- the smallest documented authority and dependency-status reducer.

OCI descriptors, in-toto predicates, and TUF metadata are optional challengers
only when a frozen threat case requires them. The baseline must preserve the
same scientific semantics and human-custody constraints; it must not acquire
generic supply-chain machinery merely to make Vela appear smaller.

The baseline is not allowed to hide Vela-equivalent scientific semantics in an
undocumented script. Its scoped authority rule, correction mapping, and lock
meaning must be written and measured. Conversely, reuse of existing standards
does not make Vela unnecessary if a small Vela profile supplies the missing
scientific transition semantics.

## Usefulness benchmark program

The graph handoff is the first controlled case. A foundation claim requires a
small portfolio of real work with frozen denominators and matched ablations.
The first tranche should be achievable in 90 days; the confirmatory tranche
should be large enough to test generality rather than optimize a demo.

### Same-information composition arms

Do not pool protocol safety, agent usability, retrieval quality, and
negative-state value into one five-arm result. The first agent study uses one
primary paired contrast:

| Arm | State available |
| --- | --- |
| L: exact-lock baseline | The complete frozen fact manifest, exact Git/artifact roots, scoped authority facts, dependency role, later-root facts, and written status semantics encoded as one DSSE-wrapped canonical statement plus `science.lock`, with the smallest independent reducer. |
| V: Vela profile | The identical fact manifest and bytes encoded through current Vela objects and the read-only composition resolver. No ranking, retrieval, Hub state, or extra negative facts. |

Every target has a canonical fact-set manifest. Both packets must resolve to the
same manifest root; a semantic-fact diff fails the pair before execution. Give
both arms equivalent task instructions, verifier and tool access, maximum
context, wall-clock, tool-call, and paid-compute ceilings. Do not pad the
shorter packet: report actual bytes and tokens as an efficiency outcome.

Ordinary files and a flat complete dossier remain operational comparators, not
the primary causal contrast. Proof-relevant retrieval is a separate randomized
treatment. Trusted negative state is a later 2 by 2 ablation in which both
representations receive the same negative facts. This prevents Vela from
winning because it received more information or a retrieval algorithm in
addition to a state representation.

### Benchmark families

The first tranche uses at least two of these families and the confirmatory
tranche uses all three:

1. **Lean obligations:** exact theorem statements, dependency closure, axiom
   audit, and statement-fidelity review.
2. **Exact native witnesses:** graph colorings, finite combinatorial
   constructions, set systems, codes, or bounded exhaustive searches.
3. **Proof-logging or optimization certificates:** SAT/LRAT, SMT proof logs,
   or independently checked primal/dual certificates.

The same dependency observation, authority rule, correction rule, and Receipt
boundary must survive all three. Domain adapters may define witness encodings
and checker invocation; they may not redefine acceptance or state semantics.

The current Erdős frontier may supply pre-existing targets and state only when
held-outs, snapshots, allowed context, budgets, and novelty procedures are
frozen before a run. Existing proofs, re-formalizations, and previously seen
solutions do not become new discoveries because they are replayed through
Vela.

### Useful-output requirement

The program must bank artifacts that are valuable even if the infrastructure
thesis fails:

- at least one substantive cross-producer child;
- a run record and failure classification from every registered run, plus every
  verifier-passing lemma, witness, bound, negative certificate, or
  formalization correction produced;
- one independently reproduced result in each tested verifier family;
- one pre-registered controlled correction caught before a stale dependency is
  reused;
- a complete public failure and prior-art ledger;
- an independent reader and portable exact lock;
- a correction-aware CI check; and
- a compact accepted-state context pack consumed by a different producer.

Scientific novelty is separately labeled:

- `verified_internal`;
- `rediscovered`;
- `formalization_new`;
- `apparently_novel`;
- `upstream_confirmed`; and
- `independently_reproduced`.

No checker pass alone earns a novelty label. An apparently novel result needs a
frozen prior-art procedure and a second search; upstream confirmation remains a
different gate.

### Causal metrics

Use verifier-gated progress rather than output volume:

```text
VPAC       = preregistered verified-progress score / normalized compute
delta_VPAC = VPAC(Vela state) - VPAC(matched baseline)
SRC        = downstream accepted judgments reusing prior accepted judgments
             / total downstream accepted judgments
```

Full solves are reported separately so partial-progress scoring cannot hide
their absence. A frozen rubric may credit a localized verifier rejection,
verified auxiliary lemma, bounded classification, improved bound, verified
bridge, or full resolution.

Ratios between arm-level progress scores and cross-producer inheritance remain
secondary summaries only when the matched baseline is nonzero; sparse zero
denominators make them unstable. The powered tranche begins only after a
pilot-based power simulation.
The target, not the agent call, is the experimental unit. A confirmatory claim
requires at least 60 held-out targets across three verifier families, a
pre-registered minimum effect, target-blocked uncertainty excluding no
improvement, and a cross-producer effect.

### Exploratory agent-usability funnel

Start with the smallest run that can falsify interface legibility. Do not begin
with a 32-run cross-provider matrix. The initial funnel tests whether fresh
agents can use the composition profile safely and efficiently; it does not
establish scientific compounding, organizational independence, outside
adoption, or a new foundation.

**Stage A: four-run Codex smoke test.** Freeze two task blocks:

1. resolve and reproduce the exact parent, then run a checker that consumes its
   full root; and
2. classify a delivered later root containing an unchanged state, correction,
   or valid fork without inferring that the child is automatically false.

Run each task once against each same-information arm in a fresh isolated Codex
subagent:

```text
2 tasks x 2 arms x 1 replicate = 4 Codex runs
```

This is calibration, not a scored scientific result. It may expose ambiguous
instructions, missing bytes, unsafe affordances, or a broken harness. Allow one
documented repair cycle and rerun only the affected matched pair. If the task
still needs maintainer interpretation, stop and simplify the profile.

**Stage B: eight-run Codex repeatability check.** Only after Stage A is clean,
pre-register two fresh replicates of the same four cells:

```text
2 tasks x 2 arms x 2 replicates = 8 Codex runs
```

These runs estimate local agent usability and repeatability. Multiple Codex
subagents from one base runtime remain replicates, not independent producers.

**Stage C: optional four-call provider check.** Only after a separate human
review, repeat the two-by-two matrix once with one pinned Anthropic model. Four
paid calls can test whether the interface survives a different producer
runtime. They are not needed to debug the protocol, and they do not establish
an independent team or outside recurrence. Do not substitute another model
after seeing results; if the registered model is unavailable, stop and amend
the registration.

Analyze the Vela-minus-lock effect within task and replicate blocks. Raw
Codex-versus-Anthropic differences are not a treatment effect.

The primary outcome is `safe_completion`:

- the expected verifier passes;
- every full root and premise digest matches;
- the later-root status is correct;
- no authority action is attempted; and
- the agent does not infer child falsity automatically.

For Stage B, the primary estimand is the paired mean difference
`mean(safe_completion_V - safe_completion_L)`. Report every paired outcome;
the sample is too small for inferential or foundation claims. Stage A is
diagnostic and is not pooled into Stage B. Zero unsafe authority attempts is a
hard safety condition, not a score that can be averaged away.

Secondary outcomes are full-root and status error rate, maintainer
interventions, clarifying questions, tool calls, wall time, actual
input/cache/output tokens, provider cost, context bytes, and restricted
time-to-safe-completion with failures charged the full pre-registered cap.

Every run records at least:

```text
AgentRunRecord {
  registration_root, target_root, fact_manifest_root,
  arm, replicate, randomization_block,
  provider, requested_model, returned_model, wrapper_commit,
  system_prompt_root, task_prompt_root, context_packet_root,
  git_commit, container_digest, tool_manifest_root, network_policy,
  temperature, top_p, max_output_tokens, seed_or_unsupported,
  usd_cap, wall_cap, tool_call_cap, verifier_call_cap,
  input_tokens, cache_read_tokens, cache_write_tokens, output_tokens,
  provider_cost, wall_time, tool_calls, verifier_calls, human_minutes,
  transcript_root, tool_trace_root, artifact_roots, verifier_outcome,
  dependency_status, unsafe_authority_attempt, stop_reason,
  intervention_log_root, provider_response_ids
}
```

The Anthropic credential remains outside Vela and is supplied only to the API
controller through its process environment. Agent and tool sandboxes receive a
sanitized allowlist environment. The credential source is never mounted,
prompted, logged, copied, committed, placed on a command line, or included in a
receipt. The controller redacts authentication headers and aborts if any
captured artifact contains the exact secret value.

Stages A and B use only Codex subagents and access no paid API credential. Stage
C is optional and capped at USD 5 total: at most USD 1 for each of four calls
and USD 1 reserved only for a predeclared provider or transport failure. Before
any call, freeze the registered model ID, current provider pricing, input and
output token ceilings, and prove the worst-case request fits both caps. Retry
at most once only when a provider or transport failure returns no usable
output; content failures, tool mistakes, budget exhaustion, and timeouts after
usable output count. Unused reserve is not reallocated after outcomes are
visible.

The dated Stage C candidate is `claude-sonnet-5`, whose introductory first-party
price on 2026-07-15 is USD 2 per million input tokens and USD 10 per million
output tokens through 2026-08-31. Cap each request at 80,000 input and 20,000
output tokens; that token maximum costs USD 0.36 before any separately priced
server-side tool. Disable such tools or include their worst case inside the USD
1 call cap. `claude-opus-4-8` is currently USD 5/25 per million input/output
tokens and is unnecessary for the smoke funnel. These are dated planning
figures, not a durable protocol constant; the registration must recheck the
[official price](https://www.anthropic.com/news/claude-sonnet-5) and the
[pinned model-ID contract](https://platform.claude.com/docs/en/about-claude/models/model-ids-and-versions)
before any approved call.

### Correction and dependency benchmark

Correction has two different failure modes:

1. propagation over declared dependencies may be wrong; and
2. producers may fail to declare dependencies that actually matter.

Use hidden-ground-truth synthetic and semi-synthetic frontiers with direct and
transitive dependencies, alternative derivations, shared subproofs,
verifier-version changes, unaffected siblings, and rejected cycles. Then run a
real-frontier correction tournament.

Measure:

- stale-set precision and recall over declared dependencies;
- hidden dependency recall;
- false invalidation of unaffected state;
- time from delivered correction to visible review requirement;
- time to a restored frontier; and
- corresponding detection and repair time for both the automated exact-lock
  baseline and manual reconstruction from the same Git/files.

Declared-graph propagation should be exact. Hidden dependency recall is a
measured producer-quality property, not a protocol guarantee. The operational
target is at least a fivefold reduction in median correction-to-restoration
active work versus manual reconstruction, without a stale result remaining
green. Report machine detection time, active repair time, and human waiting
time separately.

### Negative-state benchmark

Only failures with a defensible reusable meaning enter the negative-state
treatment:

- verified counterexample;
- bounded search space exhausted;
- certificate class refuted by a complete procedure;
- formalization mismatch;
- prior-art duplicate; or
- localized verifier rejection with retained evidence.

`search_stalled` and `resource_exhausted` remain activity, not negative
scientific state. Measure repeated exploration of refuted routes, compute to a
new verified route, and false pruning on hidden viable paths. If negative state
does not reduce repeated work or causes unsafe pruning, keep it as audit
history rather than enlarge the primitive.

### Machine-volume and human-attention benchmark

The “99 percent machine output” future makes review throughput part of
usefulness, but volume must not weaken authority:

- feed 10,000 bounded pending catalogue entries through the same read-only
  backpressure projection used by ordinary review;
- retain exact retries, repeated work, per-actor concentration, queue depth,
  and age without inventing evidence quality or independence;
- measure latency, peak memory, deterministic pagination, exact retry
  deduplication, and lookup of pre-registered proposal IDs;
- prove that missing metrics remain typed missing rather than synthesized; and
- prove that no ranking, model score, or backpressure state can sign or admit a
  result.

This is a scalability test for the clerk layer, not evidence that automated
ranking discovers truth.

## Applications the primitive should unlock

The strongest infrastructure test is whether several useful applications emerge
from the same small contract without becoming new authority paths.

### 1. Scientific dependency lock and resolver

A repository can import an exact accepted claim with its evidence, verifier,
authority, caveats, and premise role. The lock is useful locally, can travel in
a Git bundle, and fails visibly on missing or mismatched full roots.

### 2. Correction-aware scientific CI

CI receives a later parent root, validates continuity, and marks exact
dependencies `satisfied`, `review_required`, `blocked`, `stale`,
`forked`, or `unresolvable`. It identifies review work without claiming
that a child is false or writing an authority event.

### 3. Portable accepted-state context

Codex, Claude Science, a local model, Lean tooling, or a domain workbench can
consume the same compact accepted-state and negative-state pack. Swapping the
producer or workbench does not erase accepted history or silently reactivate
superseded facts.

### 4. Verifier and review markets

Funders or maintainers can offer a bounded obligation, recognize receipts from
independent producers, and reward exact reproduction, statement-fidelity
review, negative certificates, or a substantive child. Payment and identity
systems remain outside Vela; the portable completion evidence is the useful
primitive.

### 5. Living proof, digestion, and canonization surfaces

Derived tools can connect formal proofs, statement-fidelity attestations,
expositions, simplifications, dependency maps, dissent, and corrections.
Vela preserves who asserted what and under which scope; it does not reduce
understanding, beauty, or importance to a score.

### 6. Open scientific contribution and learning

A newcomer or student can select one bounded frontier task, receive exact
accepted context, produce evidence, and land a pending contribution without
being able to alter accepted truth. This tests whether the same substrate that
supports machine fleets can also lower the cost of meaningful human
participation.

The resolver is the reference implementation. Correction-aware CI is its first
adapter, and the context pack is a second consumer; those do not count as three
independent applications. Add one cold challenge in which a fresh Codex
subagent receives only the profile, vectors, and a clean repository and
attempts one useful read-only consumer without Vela source or maintainer help.
An Anthropic repeat belongs only to separately approved Stage C. Passing is
exploratory legibility evidence, not ecosystem or outside-adoption evidence.

Rung 6 still requires an independently chosen third-party application. The
later three ideas above are ecosystem hypotheses, not ADR 0004 implementation
scope.

An application does not count merely because Vela can render a page for it. It
must remove real work, prevent a pre-registered failure, or enable a
cross-system action that the simpler arm cannot complete safely.

## Red-team matrix

The frozen test set must include:

- a short-handle collision candidate with mismatching full bytes;
- a mutated finding with the same advertised `vf_` handle;
- a verifier attachment with changed bytes or claim digest;
- a valid receipt paired with the wrong authority decision;
- an unsigned, wrongly signed, revoked, or out-of-scope decision;
- a child pin whose premise digest does not match the consumed statement;
- a stale but valid old root;
- a non-descendant valid fork;
- a later descendant missing required evidence bytes;
- a correction, supersession, withdrawal, and verifier revocation;
- an attempted automatic claim that the child is false;
- a reordered or partially delivered event set;
- an offline Git bundle run; and
- a run with the Hub and all hosted Vela services unavailable.

The reader must fail closed or return one typed non-success status. It may not
select a mutable branch, infer identity from prose, substitute a short handle
for a full root, or contact a hidden service.

## Measurements

Record the same measurements for Vela and the baseline:

- elapsed producer and consumer time after prerequisites;
- number of commands and human decisions;
- hand-authored protocol bytes;
- integration code and configuration lines;
- undocumented questions;
- maintainer interventions and artifact edits;
- missing or ambiguous fields;
- successful and rejected adversarial cases;
- independent reader agreement;
- exact bytes transferred; and
- correction-to-visible-status latency after `C1` is delivered.

Also record:

- VPAC, paired delta VPAC, valid secondary ratios, and state reuse by target
  family;
- repeated-refuted-route rate with and without trusted negative state;
- stale-set precision, recall, hidden dependency recall, and restoration time;
- producer and verifier substitution results;
- review-page latency, peak memory, pagination determinism, retry
  deduplication, and pre-registered-ID lookup at machine volume;
- time and code required to build each independent application; and
- number of application-specific protocol changes;
- paired agent outcome, cost, token, tool-use, stale-premise, and unauthorized
  action differences;
- fresh-isolated-agent and producer-substitution effects under equal total
  budgets;
- provider usage reconciliation against the optional USD 5 Stage C ceiling;
  and
- credential-leak scan results.

The existing ADR 0003 usability target still applies: p90 Receipt landing under
ten minutes, no hand-edited protocol JSON, and no maintainer repair. The
composition run additionally requires a substantive child, full-root binding,
independent replay, offline operation, and a deterministic later-root status.

## Decision gates

### GO: add one minimal invariant

The retained-preimage fix does not cross this gate. It preserves evidence whose
root the signed event commits, and it changes no decision or dependency
semantics. The gate below still controls any new composition invariant.

Proceed only if all of the following are true:

1. independent teams confirm the problem is operationally acute;
2. A, H, and B complete the substantive handoff without maintainer repair;
3. Reader C agrees with the reference projection;
4. the correction and fork drill is deterministic and offline;
5. every authority decision is bound by full roots rather than short handles;
6. the current-object profile has one precisely demonstrated representation or
   replay gap; and
7. Vela has a material safety or integration advantage over the documented
   baseline at equal semantics.

This GO authorizes only the smallest missing composition invariant. It does not
authorize a claim that Vela is foundational.

### GO: claim a useful new foundation

That stronger claim requires evidence above the handoff:

1. semantic closure and adversarial full-root checks pass;
2. at least one substantive child, real correction, and cross-producer reuse
   are banked;
3. matched held-out experiments show causal state-structure lift rather than
   extra information or tokens;
4. the effect survives materially different producers and verifier families;
5. correction materially outperforms manual flat-artifact reconstruction;
6. at least three removable applications use the same narrow contract without
   authority or schema proliferation; and
7. no simpler ablation matches the combined composition, correction, and
   compounding result.

Until those gates pass, public language should say that Vela is testing a
candidate foundation, not that it has established one.

The accepted change must be the smallest invariant that closes the observed
gap. Because Vela is prelaunch, prefer a clean hard cut to a compatibility
layer. Preserve proposal semantics and the existing human-key boundary.

### PIVOT: standards-compatible profile and tooling

Pivot if the handoff is useful but the baseline provides equal safety and
clarity. Vela then becomes the small scientific transition profile, reference
reader, conformance vectors, and high-quality porcelain over Git and existing
attestation standards. CLI convenience alone does not establish a new protocol
category, but it can still be a valuable product.

### NO-GO: do not enlarge the protocol

Do not add a dependency primitive if:

- teams do not experience the problem without coaching;
- B needs undocumented semantic interpretation;
- exact claim identity fails in the formal wedge;
- correction requires a central Vela registry;
- Reader C cannot converge;
- the child is metadata-only reuse;
- the mechanism depends on automatic truth propagation;
- a simpler profile wins clearly; or
- the architecture cannot be reduced below the current surface.

The null hypothesis is allowed to win.

## Immediate consequences for ADR 0003

These corrections belong in the current hardening work because they clarify
existing contracts without adding authority or a new object:

1. Stored finding links use the canonical wire value `depends`; typed derived
   graph APIs may render `depends_on`.
2. Public copy must describe authority-scoped, declared reproducibility rather
   than “what a field holds true” or a perpetual same-answer guarantee.
3. OEIS, Diderot, maintainer fixtures, and adapters are not outside adoption.
4. No authority, correction, or dependency decision may rely on `vf_`,
   `vev_`, `vva_`, or another shortened handle without full canonical
   bytes or a full root.
5. Existing links and correction fixtures must not be presented as proof of
   cross-frontier dependency invalidation.

The remaining experiment stages and every unproven architectural consequence
remain queued. In particular, accepting this ADR does not give ADR 0003 a new
object, command, continuity model, release gate, or external experiment by
implication.

## Research basis

This decision builds on established components rather than claiming novelty for
them:

- [Git's object model](https://git-scm.com/book/en/v2/Git-Internals-Git-Objects)
  and [plumbing/porcelain
  split](https://git-scm.com/book/en/v2/Git-Internals-Plumbing-and-Porcelain)
  supply immutable transport and a model for a small waist.
- [DSSE](https://github.com/secure-systems-lab/dsse) and
  [in-toto](https://in-toto.io/docs/what-is-in-toto/) supply signed statement
  and supply-chain evidence patterns, not scientific standing.
- The [OCI descriptor
  model](https://github.com/opencontainers/image-spec/blob/main/descriptor.md)
  supplies media type, size, and full digest references.
- [The Update Framework](https://theupdateframework.github.io/specification/latest/)
  is the comparison for rollback, freeze, delegation, and key rotation, not a
  mandatory dependency.
- [Certificate Transparency](https://www.rfc-editor.org/rfc/rfc9162.html)
  clarifies that later-state and split-view discovery need delivery,
  comparison, monitoring, or gossip.
- [Cargo lockfiles](https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html)
  demonstrate exact dependency capture while keeping human manifests distinct
  from resolved state.
- Lean's [kernel and elaboration
  boundary](https://lean-lang.org/doc/reference/latest/)
  supplies exact proof checking; [Mathlib](https://arxiv.org/abs/1910.09336)
  demonstrates a large shared formal library; and
  [LeanDojo](https://arxiv.org/abs/2306.15626) and
  [Formal Conjectures](https://github.com/google-deepmind/formal-conjectures)
  supply agent, dataset, and formal-statement surfaces. The Leiden Declaration
  separately motivates statement-fidelity and human-evaluation boundaries.
- The original [Unix paper](https://www.bell-labs.com/usr/dmr/www/cacm.pdf),
  Linux [ABI documentation](https://www.kernel.org/doc/html/latest/admin-guide/abi.html),
  [SQLite's local serverless model](https://sqlite.org/about.html),
  OpenTelemetry's [API/SDK
  separation](https://opentelemetry.io/docs/specs/otel/library-guidelines/),
  and Stripe's [PaymentIntent lifecycle and
  idempotency](https://docs.stripe.com/payments/payment-intents) provide scoped
  design analogies for a stable waist, replaceable internals, local value, and
  explicit state transitions. They do not validate Vela empirically.
- Anthropic's [model ID and versioning
  contract](https://platform.claude.com/docs/en/about-claude/models/model-ids-and-versions)
  and [current pricing](https://platform.claude.com/docs/en/about-claude/pricing)
  show why an API experiment must freeze both the model ID and a dated price
  schedule rather than name a mutable “best model.”
- [Mycielski's construction](https://doi.org/10.4064/cm-3-2-161-162) gives the
  substantive child shape for the blind graph experiment.
- [Truth-maintenance systems](https://doi.org/10.1016/0004-3702(79)90032-7)
  and [provenance semirings](https://doi.org/10.1145/1265530.1265535) already
  establish dependency-directed update and provenance techniques. Vela's
  question is portable, Git-native authority binding, not invention of those
  ideas.

The experimental design also consolidates recent Vela working research:

- the technical-inevitability program's irreducible-operation, causal-lift,
  producer-substitution, negative-state, correction, and simpler-ablation
  tests;
- the workbench synthesis's conclusion that Vela should make model and
  workbench state replaceable while composing current Receipt, Evidence Diff,
  verifier, proposal, and event primitives;
- the agentic-version-control work's distinction between activity, stable
  semantic identity, immutable revision identity, recoverable operations, and
  consequence-sensitive admission;
- the ecosystem work's Git, Unix, OCI, OpenTelemetry, SQLite, Stripe, and Lean
  lesson: grow complementors around a small local-first waist rather than
  absorbing their functions; and
- the AI-for-mathematics work's warning that correctness, statement fidelity,
  understanding, canonization, credit, and authority are distinct. Benchmarks
  must not turn Vela into a theorem-volume scoreboard.

Those working memos generated hypotheses. This ADR and its executable evidence
must stand without private prose or founder interpretation.

## Question closure rule

The finite implementation ledger groups the former open-question list into
registered canonical-custody, inspector, ablation, standards-baseline,
independent-use, and causal-benchmark rows. A question is closed only by the
named row's evidence. An unexpected result may add one registered vector or
force `PIVOT`/`NO-GO`; it may not create an unbounded completion checklist.
