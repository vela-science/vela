# ADR 0017: Kernel, Frontier Algebra, and Discovery Calculus boundaries

- Status: Proposed
- Protocol effect: None
- Candidate release: No Vela release required for the first experiment
- Scientific authority effect: None

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

The Kernel does not own probability, confidence, importance, ontology
inference, heuristic transfer, information gain, opportunity ranking, or a
global truth score.

### 2. Frontier Algebra is a disposable exact projection

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

The algebra's exact initial outputs are:

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

Before presenting new results:

1. Keep public `THEORY.md` as the normative formal-boundary document.
2. Split protocol and research axiom registries.
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

Historical commits and reports remain available. Current documentation must
not present experimental statements as shipped protocol guarantees.

## Evidence gates

### Frontier Algebra vertical slice

Use one exact Erdos statement/proof/fidelity case. The slice passes only if:

- every atom resolves to exact retained Vela evidence;
- direct runtime evaluation agrees with frozen formal fixtures;
- a correction removes the affected stronger route without erasing the
  narrower formal theorem;
- minimal environments and shared-origin warnings are complete within declared
  bounds; and
- a user answers "why does this stand and what would unsettle it?" more
  accurately than from the current graph alone.

### Discovery Calculus vertical slice

Use a finite Sidon bound interval. The slice passes only if:

- the state, lens, action catalogue, verifier, prior, likelihood, and output are
  root-bound;
- the finite-uniform special case and sequential accounting fixtures agree;
- null verifier and authority outcomes are represented;
- ranking sensitivity to defensible model changes is disclosed; and
- the result is labeled information under the lens, never impact or truth.

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
