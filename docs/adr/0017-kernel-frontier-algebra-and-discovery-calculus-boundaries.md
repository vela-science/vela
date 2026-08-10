# ADR 0017: Kernel, Frontier Algebra, and Discovery Calculus boundaries

- Status: Deferred — research only
- Target-surface disposition: any Vela Target Index or `next`/`start` language
  below is historical; commit `719cbc77` retired that core surface on 2026-08-10.
- Protocol effect: None
- Candidate release: No Vela release required for the first experiment
- Scientific authority effect: None
- Current disposition: Preserve the layer analysis, but do not implement a
  Frontier calculus, universal work graph, or new release surface until a
  repeated evidence gap is demonstrated.

## Context

Vela already has a narrow useful invariant: exact evidence enters through a
Receipt, becomes a proposal, is checked by named verifiers, is routed by an
authorized policy or protected human decision, and is replayed from immutable
events. That Kernel separates integrity, reproduction, verification,
acceptance, and publication.

The current internal frontier-calculus research mixes that shipped boundary
with several stronger claims. Exact review reproduced material gaps:

- a list-of-lists syntax is described as the free commutative semiring even
  though it is not quotiented or law-carrying;
- support is inferred from variable occurrence, which mishandles the
  multiplicative unit;
- a max-product best-route score is called confidence without calibration;
- a product bilattice visualization is called canonical beyond what its
  representation theorem establishes;
- deterministic duplicate rejection is described as consensus or Sybil
  resistance;
- a trusted Boolean fold is described as a succinct proof without a
  cryptographic construction; and
- an ad hoc scalar opportunity score combines incomparable quantities.

Those overclaims are unnecessary. The useful program is smaller: derive exact
justification and correction explanations from a pinned Vela state, then let
optional declared models rank future actions. Neither layer gains authority.

## Decision

Adopt three named layers:

```text
Vela Kernel
  exact objects, authority, transitions, correction, and replay

Frontier Algebra
  exact read-only justification and correction derivations

Discovery Calculus
  optional root-bound information and decision lenses
```

The dependency direction is one-way. Each upper layer may read exact outputs
from the layer below. It cannot mutate, sign, accept, or silently reinterpret
the lower layer.

### 1. Vela Kernel remains narrow

The Kernel owns only protocol facts and deterministic projections, including:

- canonical bytes and root families;
- Receipts, artifacts, proposals, verifier attachments, actors, policies, and
  signed events;
- protected human and exact policy authority;
- deterministic replay, proposal-decision parity, and correction without
  erasure; and
- transaction and publication separation.

The Kernel may retain and root released confidence, transfer, attestation, and
other scientific-state bytes required for exact replay. It does not define a
universal probabilistic meaning or calibrated-confidence doctrine for those
bytes, and it does not own importance, ontology inference, heuristic transfer,
information gain, opportunity ranking, or a global truth score.

### 2. Frontier Algebra is a disposable root-bound projection

For an exact claim revision and explicit context, derive separate supporting
and opposing justification circuits:

```text
J+(claim, context)
J-(claim, context)
```

The circuit language is closed:

```text
zero
one
atom(typed_exact_root)
or(children)
and(children)
```

`one` means an unconditional derivation only relative to the projector's
explicit context and closed derivation rules. It is not global truth,
acceptance, or authority.

Runtime circuits are canonical, hash-consed acyclic DAGs with sorted children,
typed leaves, exact source roots, explicit duplicate semantics, and a projector
version. They are derived artifacts, not event or acceptance objects.

The first leaf types are:

```text
evidence
assumption
verifier_result
statement_faithfulness
certified_transfer
source_assertion
```

Authority events do not become scientific-evidence leaves. They remain a
separate Kernel standing projection.

The projector also keeps distinct justification planes for scientific
support, scientific opposition, artifact integrity, verifier reproduction,
statement faithfulness, and transfer validity. A stronger resolution claim
may require judgments from several planes, but it does not multiply them into
one undifferentiated evidence score. In particular, a successful verifier,
faithfulness judgment, valid transfer, or human decision is not by itself
evidence that the underlying scientific assertion is true. Review-event and
authority-root material cannot be emitted as scientific-evidence atoms.

The algebra's initial outputs are exact only relative to the pinned source,
closed rooted derivation rules, projector version, and canonicalization policy:

- support and opposition existence using circuit nonzeroness;
- `unresolved`, `supported`, `opposed`, or `contested` standing;
- subset-minimal support and opposition environments;
- shared-origin warnings;
- single-atom blast radius and bounded minimal cut sets;
- surviving routes after an exact correction;
- bounded repair sets; and
- exact transfer paths and assumptions.

Every output binds the source Frontier identity, Git commit/tree, event root,
scientific-state root, exact claim revision, projector version, completeness
status, and output root.

For a Profile v1 source, the projector first validates the complete repository
context from ADR 0016 and binds the full `frontier_id`, `identity_root`,
`dependency_root`, and `scientific_state_root`. Membership in
`vela.scientific-state.v2` is a source commitment, not proof that an object is
accepted or is scientific evidence; the projector derives role and standing
from the exact event and object semantics. Likewise, a Profile v1 dependency
pin is retrieval and context identity only. It cannot become a support atom,
transfer edge, or standing claim without a separately retained, exact
class-specific relation.

Before a circuit root is portable, the Phase 1 specification must freeze:

- `or([]) = zero` and `and([]) = one`;
- same-operator flattening rules;
- whether and where duplicate children retain multiplicity;
- typed-atom encoding and domain-separated node hashing;
- canonical child ordering; and
- the behavior of Boolean, natural-number, and minimal-environment readings
  when multiplicity is present.

The projector output additionally binds the closed derivation-rule schema and
root, projector implementation digest, source component roots, circuit
canonicalization policy, and exact complete or truncated limits. It fails
closed on dirty or unrooted input, a missing exact reference, an unknown
state-carrying relation, a cycle in a relation declared acyclic, or replacement
of an exact ID with a label, embedding, or short digest.

### 3. Polynomial semantics are denotational, not the runtime store

The formal denotation uses Mathlib's `MvPolynomial Atom Nat`. A small
`ProvExpr` and the runtime DAG expand into that denotation. Required theorem
targets are:

```text
direct evaluation = polynomial evaluation
circuit evaluation = unfolded-expression evaluation
retraction = substitution of selected atoms by zero
retraction cannot invent a Boolean derivation
minimal environment avoids Y iff the claim survives retracting Y
Y hits every minimal environment iff Y removes all current routes
```

Useful algebraic lenses must name their target algebra, leaf valuation,
sharing policy, and validity conditions. Natural-number evaluation counts
represented derivation trees, not independent confirmations. Max-product is a
best-route score, not confidence. Probabilities require an explicit dependence
model and calibration evidence.

### 4. Transfer classes remain distinct

The read model distinguishes:

```text
representation_equivalence
witness_transformation
logical_reduction
formalization_relation
causal_transport
heuristic_analogy
```

Only the effect allowed by the declared class may propagate. Heuristic analogy
never transports standing. A transported support route contains the exact
source route, transfer certificate, and every declared assumption. Retracting
any required factor removes only that route.

### 5. Discovery Calculus is lens-relative

A Discovery lens binds a pinned Vela state, a declared resolution space,
available actions, outcome and verifier models, governance outcomes, cost,
risk, and optional utility. It reports:

```text
best under lens L
at Frontier root R
under assumptions A
with model uncertainty U
```

For action `a`, let `Y_a` be the raw result and let `Z_a` be the result after
the named verifier and authority route, including an explicit null outcome for
invalid, unverifiable, out-of-scope, deferred, or rejected work. Expected
accepted information is:

```text
I(Theta ; Z_a | S, L)
```

This is a property of the declared model, not an intrinsic number attached to
an artifact. In a finite uniform hard-elimination lens it reduces to:

```text
log2 |Omega(S)| - log2 |Omega(S + delta)|
```

Sequential accounting conditions each increment on prior accepted outcomes.
Information, decision utility, downstream value, execution cost, verification
cost, time, safety risk, and model uncertainty remain a vector unless a named
policy declares an optimization rule.

A lens may consume a pinned Target Index v2 as an action catalogue. It does not
become the domain generator or Vela seal: lens scores and ordering remain a
separate rooted advisory artifact and cannot rewrite the candidate, sealed
index, canonical producer order, `vela next` offer, or retained
`vela.target-task-binding.v1`. Promoting one lens into domain ranking requires
the ordinary domain candidate-generation path plus the later preregistered
product gate; ADR 0017 itself grants no such promotion.

The retained operational outcome alphabet distinguishes at least verifier
rejection, verifier inconclusive, artifact unavailable, scope mismatch,
authority deferral, authority rejection, and accepted delta. A named lens may
collapse selected outcomes to a null or erasure symbol, but the underlying
projection does not erase those distinctions.

Any data-processing theorem is scoped first to a pinned automated policy whose
output is downstream of the raw/verifier result and whose routing contributes
no additional information about the hidden scientific state. A human decision
may introduce external scientific knowledge; the theorem applies only when
that input is represented in the channel model or when the required
conditional-independence premise is proved.

Reproduction equivalence belongs to the named, versioned verifier. An
assurance summary or automation envelope is a derived read projection over
existing Receipt, execution-binding, verifier, fidelity, consequence, replay,
and signed-policy facts. Neither is a new protocol object, authority status, or
automatic implication of acceptance.

### 6. No new protocol or file format

The first implementation is a source-local read projection over one frozen
Erdos claim and one finite Sidon lens. Ordinary JSON/YAML fixtures and locks
may be used as analysis artifacts. They are not Vela protocol objects and do
not enter accepted state without a normal Receipt and authority route.

Do not add an event kind, hosted database authority, universal ontology,
custom archive, raw-agent-history dependency, or Observatory mutation path.
Extract a reusable analysis package only after two independent consumers need
the same stable contract and extraction deletes duplicated code.

## Current research migration

Before presenting new results, complete the bounded reconciliation steps.
Phase 0A corrects current prose and claim status without changing theorem
surfaces. Phase 0B quarantines the active compatibility aggregate. Phase 0C
then separates protocol and research axiom reports from one membership source:

1. Keep public `THEORY.md` as the normative formal-boundary document.
2. Split protocol and research axiom reports without duplicating membership.
3. Remove PoVD, Accumulation, HeteroAccumulation, and ProtocolKeystone from the
   active core-theorem bundle. Preserve them under research history with claims
   narrowed to what the statements prove.
4. Rename PoVD as a monotone-improvement policy example; remove consensus,
   Sybil-resistance, and authority-free claims.
5. Describe the accumulator Boolean as a trusted fold invariant, not a
   succinct proof.
6. Rename `kappa_confidence` to `best_route_score`; retain historical field
   readers only where fixtures require them.
7. Mark the graded bilattice as an optional named lens and keep exact
   four-valued standing primary.
8. Replace the scalar opportunity score with an explicit action-assessment
   vector.
9. Correct the multiplicative-unit support bug.
10. Preserve falsified sheaf and transfer-amplification experiments as negative
    evidence, outside the active theory.

Phases 0A–0C are complete. They do not block Repository Profile v1 or Vela
`v0.914.0`; they block the presentation of any new Frontier Algebra theorem or
runtime result until the later evidence gates pass.

Historical commits and reports remain available. Current documentation must
not present experimental statements as shipped protocol guarantees.

## Evidence gates

### Frontier Algebra vertical slice

Use one exact Erdos statement/proof/fidelity case. The slice passes only if:

- every atom resolves to an exact retained Vela source object in its declared
  justification plane;
- direct runtime evaluation agrees with frozen formal fixtures;
- a correction removes the affected stronger route without erasing the
  narrower formal theorem;
- minimal environments and shared-origin warnings are complete within declared
  bounds; and
- a first-party observed user answers "why does this stand and what would
  unsettle it?" more accurately than from the current graph alone.

Paper, breakthrough, or broad product claims require later external use or
independent reproduction. First-party vertical-slice evidence earns no
independent credit.

### Discovery Calculus vertical slice

Use a finite Sidon bound interval. The slice passes only if:

- the state, lens, action catalogue, verifier, prior, likelihood, and output are
  root-bound;
- the finite-uniform special case and sequential accounting fixtures agree;
- finite nonnegativity and hard-elimination telescoping are proved for the
  declared model;
- finite expected Bayesian surprise agrees with mutual information;
- null verifier and authority outcomes are represented;
- ranking sensitivity to defensible model changes is disclosed; and
- the result is labeled information under the lens, never impact or truth.

The first Sidon slice is diagnostic only. Product-ranking promotion requires a
later preregistered pilot against random and cost-only baselines, with the
selected metric, model, and falsification rule frozen before actions run.

Failure of the Discovery slice does not invalidate the Frontier Algebra.

## Stop rules

Stop or narrow the work if circuit construction requires large manual semantic
annotation, minimal environments are unusably explosive without an honest
bounded representation, users gain no correction insight over an ordinary
graph, or typed justification cannot remain separate from authority.

Stop the information lens if arbitrary priors dominate rankings, erasure models
cannot be grounded, rankings are unstable under small defensible changes, or
users prefer explicit obligations and cost. Do not pursue cryptographic
accumulation until a real selective-replay need beats signed checkpoints and a
concrete proof system binds the actual Vela transition relation.

## Alternatives rejected

### Put the calculus in the Kernel

Rejected. It would turn model-relative explanation and ranking into protocol
surface and blur the authority boundary.

### Keep the current grand unified narrative

Rejected. Several headline claims outrun their formal statements and weaken
the defensible contribution.

### Delete the research history

Rejected. Failed and narrowed experiments are useful evidence. They become
clearly historical rather than active guarantees.

### Build a graph database or ontology first

Rejected. One exact correction-aware slice must prove the derivation rules and
user need before another persistent system exists.

## Consequences

Vela retains one simple product story: the Kernel records exact scientific
state; the Frontier Algebra explains why it stands and what correction changes;
the Discovery Calculus helps choose the next action under an explicit model.
The two analysis layers can become mathematically serious without acquiring
scientific authority or burdening ordinary Frontier adoption.
