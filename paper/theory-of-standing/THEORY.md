# A Theory of Scientific Standing

Three separations that scientific infrastructure cannot express, each stated
as a theorem, each witnessed by a measurement from a live corpus.

**Status: draft. The mathematics is deliberately light; the genre is the CAP
theorem and the end-to-end argument — systems principles whose value is the
frame and the witness, not the proof depth. Every witness below is a
committed, reproducible artifact.**

---

## 0. Why theorems

Vela's design principles — only an attributed Decision changes Standing;
authorities stay plural; corrections propagate by computation — have to date
been stated as engineering discipline. This note restates them as three
formal separations, because a principle can be argued with but a separation
with a witness can only be worked around. Each theorem is mathematically
modest. Each is, to our knowledge, unstated in the literature of scientific
infrastructure, and each was instantiated *empirically* on
google-deepmind/formal-conjectures during 2026-08-23/24.

Notation. `C` claims, `A` authorities, `E` environments, `Σ` a status
vocabulary. Source bytes `s ∈ S`. Elaboration `elab : S × E ⇀ Prop` maps
source and environment to the proposition actually stated.

---

## 1. Standing is a relation, not a property

**Definition 1.** A *status system* assigns `status : C → Σ` — one value per
claim (the status column of every registry, database, and badge in current
use). A *standing system* records a relation `R ⊆ A × C × Σ` together with
an append-only history of the attributed events that produced each triple.

**Premise (empirical).** The standing relation of real mathematics is not
functional in `C`: there exist claims on which distinct authorities hold
distinct statuses *while both are correct under their own published policy*.

*Witness.* `Erdos730.erdos_730.variants.explicit_pairs` at Formal
Conjectures commit `e51535ae`: FC records it *proved (textbook)*; the
Palomar registry's published `permitted_axioms` policy classifies the same
proof *inadmissible* (its closure contains `Lean.ofReduceBool`,
`Lean.trustCompiler`). Neither is in error; no record relates them.

**Theorem 1 (No-collapse).** Any status system evaluated on a claim where
the standing relation is non-functional misreports at least one authority's
judgment. Consequently a single-valued status column can represent real
scientific standing only by information loss or by fabricating a consensus
that does not exist.

*Proof.* A function `C → Σ` cannot agree with a relation containing
`(a₁, c, σ₁)` and `(a₂, c, σ₂)` with `σ₁ ≠ σ₂` at `c`. ∎

The mathematics is a pigeonhole; the content is the premise — demonstrated,
not assumed — plus the design consequence: adequate infrastructure must
store the relation and the attributed events, and may *derive* per-authority
views, never the reverse. This is Vela's `Standing`-through-`Decision`
semantics stated as an expressiveness requirement rather than a preference.

---

## 2. Correction is typed computation, and untyped graphs get it wrong both ways

**Definition 2.** A *dependency record* over results is a graph `G`; a
*trust-typed* record labels each edge with the basis it transmits (kernel
proof, compiler trust, statement-completeness, environment pin, authority
policy). For an environment change `δ` with support in a set of basis
types, the affected set is

- untyped: `Aff_u(δ)` = all descendants of any node touching the changed
  environment;
- typed: `Aff_t(δ)` = descendants reachable along edges whose type lies in
  `δ`'s support.

**Theorem 2 (Blast-radius separation).** `Aff_t(δ) ⊆ Aff_u(δ)` always, and
the inclusion is strict whenever co-located dependencies of distinct types
exist. Hence any invalidator without trust-typed edges is, on such corpora,
either *unsound* (misses consequences by ignoring the environment edge) or
*maximally over-approximate* (invalidates every descendant of the
environment node).

*Proof.* Inclusion is monotonicity of reachability under edge-subset
restriction; strictness is exhibited by any node depending on the changed
environment through a type outside `δ`'s support. The dichotomy: an untyped
invalidator either includes the environment node's out-edges (then
`Aff_u` = all dependents, over-approximation) or excludes them (then it
misses every genuine trust consequence, unsoundness). ∎

*Witness, measured.* The Lean 4.32.2 toolchain bump (#4428, merged
2026-08-23). Typed consequence: 231 of 5,924 authored declarations (3.9%)
require trust re-verification (compiler-trusting proofs); kernel-only
proofs — e.g. `delta_ne_one`, whose closure is clean while `explicit_pairs`
beside it is compiler-trusting — require rebuild only. An untyped
dependency graph, in which every declaration depends on the toolchain,
invalidates 100% or misses the 231 entirely: a measured **25× separation**
between the correct blast radius and the best untyped answer.

---

## 3. A claim is not its source: meaning is environment-indexed

**Definition 3.** An identity scheme *individuates by source* if it assigns
claim identity as a function of source bytes alone (file hashes, DOIs,
URLs — the entirety of current practice).

**Theorem 3 (Base-change).** If `elab` does not factor through `S` — that
is, if there exist `s, e₁, e₂` with `elab(s,e₁)` and `elab(s,e₂)` distinct
propositions — then every source-individuating identity scheme conflates
distinct claims, and no consumer of such identities can detect the
conflation from the identities themselves.

*Proof.* Immediate: a function of `s` alone is constant across `e`. ∎

*Witness, at scale.* Across the 4.27 → 4.32.2 bump, **600 declarations
changed the proposition they state — 485 of them in modules whose source
bytes are identical across the pins, elaborated through a byte-identical
macro** (`answer(sorry)` yields `sorry ↔ P` at 4.27 and `True ↔ P` at
4.32.2). The corpus's own documentation regards the newer behavior as
intended, which sharpens rather than weakens the point: even *intended*
elaboration change re-individuates claims, and source-keyed identity —
including content-addressing by source hash — cannot see it. Adequate
identity for formal claims is at minimum the pair (source, environment);
adequate trust accounting is a function of that pair (this is the Effective
Trust Closure's fifth component, stated as necessity rather than hygiene).

---

## 4. What the three separations jointly assert

A system that stores scientific results must, to avoid provable
misrepresentation on corpora that demonstrably exist:

1. store standing as attributed relation, deriving status views (Thm 1);
2. type its dependency edges by trust basis, computing consequence per type
   (Thm 2);
3. individuate claims by source *and* environment (Thm 3).

Vela is one implementation of the conjunction. The theorems do not show
Vela is the right implementation; they show the *shape* is forced — any
system meeting all three is, whatever it is called, the same kind of object
as Vela, and any system failing one is exhibiting one of the three
witnessed failure modes. That is the strongest defensible form of the
claim "Vela is a research contribution and not an engineering preference."

## 5. Honesty section

- All three proofs are elementary. The contribution is the model, the
  separations, and the witnesses — the CAP/end-to-end genre, claimed
  explicitly.
- The witnesses come from one corpus, one ecosystem, and two toolchain
  pins; breadth is unproven.
- Theorem 2's 25× figure depends on the census's authored/generated
  classification (robust across three filters, boundary cases disclosed in
  the companion paper).
- None of this establishes adoption, usability, or that the implementation
  earns the theory. Those remain empirical questions with their own gates.

## 6. Provenance

Witness artifacts: `fc-trust-closure-v2.e51535ae.jsonl`,
`fc-trust-closure-v432.a3bc3141.jsonl`, `differential.e51535ae-a3bc3141.json`,
`stmtprobe.lean` (vela-evals); the Erdős 730 signed repository, root
`sha256:bdd0f36e…4a85`; companion papers *The Effective Trust Closure* and
*What Inherited Scientific State Is For*, same branch.

## 7. Mechanized minimal state model

The isolated [Lean proof artifact](lean/README.md) mechanizes the smaller
Submission, scoped Verification, Repository-local Decision admission, Event,
replay, correction, and Standing model that survived the later Phase III
minimality fork. It includes fail-closed admission theorems and a finite
C-versus-D lifecycle-freshness witness. The mechanization changes no runtime or
Protocol 1 contract and makes no claim of universal scientific truth,
productivity, adoption, or all-science sufficiency.
