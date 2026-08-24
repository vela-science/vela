# The Effective Trust Closure

**What a formal mathematical result actually rests on, why the standard
accounting cannot say, and a census of the flagship AI-conjecture corpus.**

Draft. All numbers are computed from committed artifacts and reproduce from
scratch; Section 11 gives exact commands. Nothing here has been reviewed by
the maintainers of any corpus or registry it discusses.

---

## Abstract

Formal mathematics has one standard trust accounting: the axiom closure of a
declaration, as reported by `#print axioms`, consumed by registry admission
policies, and relied on by benchmarks and downstream formalization efforts.
We show this accounting conflates at least four distinct trust states, each
demonstrated concretely on Lean 4.27.0, and we define the **Effective Trust
Closure** that separates them.

A census of the Formal Conjectures corpus — measured on a fork whose Lean
source directories are byte-identical to upstream
google-deepmind/formal-conjectures at `9f5ee773` (checkout commit
`e51535ae` on `williamjblair/formal-conjectures`, branch
`comparator-workspaces`) — 9,894
built declarations, of which 5,924 are authored and 3,970 compiler-generated
under the environment's own classification — finds that among the 2,862
authored declarations whose closure contains `sorryAx`, **850 (29.7%) are
statement holes**: the statement itself, not
merely the proof, is incomplete. 835 are directly visible in the
declaration's type. **15 are hidden**: the statement reads as fully stated
but transitively rests on an object *defined by choice from an unproved
existence lemma* — one mechanism, verified in the source of every one of the
six affected problem families. A further 231 authored declarations rest on
compiler trust, and 3 combine both: concrete numerical facts, proved by
native evaluation, about a sequence whose defining existence proof is
`sorry`.

No consumer of the trust reporting can see these distinctions — even where,
as with the corpus's deliberate `answer(sorry)` idiom, the authors encode
them in source. We show the conflation
already has an institutional consequence: one registry's record and a second
registry's published admission policy take contradictory positions on the
same proof, both correct under their own rules, with no record anywhere
relating them. Finally, we demonstrate the
closure operating as durable scientific state: recorded as a bounded claim in
a signed repository whose acceptance gate refused the producer's own passing
verification until an independent recomputation existed.

---

## 1. The question `#print axioms` answers, and the questions it doesn't

For a constant `d`, `#print axioms d` reports the set of axioms transitively
used by `d`'s elaborated term. Lean's ordinary base is
`{propext, Quot.sound, Classical.choice}`; `sorryAx` marks incompleteness;
`Lean.ofReduceBool` and `Lean.trustCompiler` mark native evaluation.

This single set is asked to answer at least four different questions:

1. Is the statement complete?
2. Is the proof complete?
3. What machinery is trusted beyond the kernel?
4. Would this result be admissible under a given policy?

Sections 2–5 show, with named artifacts, that it cannot answer any of them
reliably — not because the implementation is wrong, but because the set has
no vocabulary for the distinctions.

Throughout, "the corpus" is Formal Conjectures as checked out at commit
`e51535ae2caeab6c7493450a5d86a5a8651fa82d` of
`williamjblair/formal-conjectures` (branch `comparator-workspaces`), whose
`FormalConjectures/` source directories are byte-identical (`git diff` empty)
to upstream google-deepmind/formal-conjectures at merge-base `9f5ee773`;
everything is elaborated under the corpus's pinned toolchain
`leanprover/lean4:v4.27.0`. External reviewers should reproduce against
upstream `9f5ee773` or the fork commit; upstream cannot resolve `e51535ae`.

---

## 2. Unproved is not unstated

The corpus deliberately formalizes open questions whose *answer* is part of
the unknown:

```lean
theorem erdos_730 : answer(sorry) ↔ S.Infinite := by sorry
```

Here `sorryAx` enters through the **type**: the left side of the
biconditional is a hole. Compare a fully stated conjecture:

```lean
theorem erdos_23.variants.n1 :
    ∀ (G : SimpleGraph (Fin 5)), G.CliqueFree 3 → ... := by sorry
```

Here `sorryAx` enters only through the **value**. `#print axioms` reports the
two identically. The difference is material to every consumer: an unproved
statement can be proved; a statement hole cannot — it must first be
*instantiated*. For roughly 30% of the corpus's sorry-bearing targets, the standard trust
accounting cannot tell a consumer that the target is a hole rather than a
statement — the source makes it visible, the closure does not.

Detection is mechanical once the distinction is named: check whether the
declaration's **type** — directly, or transitively through the constants it
uses — reaches `sorryAx`. We validated the discriminator on a positive
control (`erdos_730`: type reaches `sorryAx` directly) and a negative control
(`erdos_23.variants.n1`: closure contains `sorryAx`, type does not) before
running it at scale.

---

## 3. Hidden statement holes: choice from a hole

The sharp class is statements whose type does **not** mention `sorryAx` yet
transitively reaches it. After excluding compiler-generated declarations
(Section 6.2), the corpus contains **15**, spanning six problem families —
and every one instantiates the same pattern:

> **An object is defined by `Exists.choose`, `ExistsUnique.choose`, or
> `Nat.find` applied to an existence lemma whose proof is `sorry`. Downstream
> theorems quantify over that object. Their statements read as fully stated;
> the object is a hole.**

Source-verified instances, one per family:

| Family | The sorried existence | The definition through it | Downstream statements |
|---|---|---|---|
| `ErdosProblems/697` | `density_exists (m) (α) : ∃ δ, HasDensity … := by sorry` | `def δ (m) (α) : ℝ := (density_exists m α).choose` | `erdos_697.parts.i/ii`, `variants.delta_lt` |
| `ErdosProblems/295` | `exists_k (N) : ∃ k n, … := by sorry` | `Nat.find (exists_k N)` | `variants.erdos_straus` |
| `ErdosProblems/961` | `erdos_961.sylvester_schur := by sorry` (well-definedness is then proved *from* it) | `Nat.find (… well_defined k hk)` | `variants.erdos_upper_bound`, `variants.jutila_ramachandra_shorey_upper_bound` |
| `ErdosProblems/1055` | `exists_p (r) : ∃ p, p.Prime ∧ IsOfClass r p := by sorry` | `Nat.find (exists_p r)` | `variants.erdos_limit`, `variants.selfridge_limit` |
| `OEIS/87719` | `a_exists (n) := by sorry` | `def a (n) : ℕ := Nat.find (a_exists n)` | `a_1`, `a_2`, `a_3`, `a_formula` |
| `Wikipedia/MovingSofa` | `ABφθSpec.existsUnique := sorry` | `def A : ℝ := ABφθSpec.existsUnique.choose.1` (Gerver's constants) | `sofaConstant_eq`, `sofaConstant_eq_volume_gerversSofa`, `volume_eq_sofaConstant_iff_congruent_gerversSofa` |

(A sixteenth candidate, `HartshorneConjecture...Splitting.iso`, reaches a
sorried upstream declaration through the same mechanism, but is a
compiler-generated field projection — flagged by the environment's own
`isProjectionFn` — and is therefore classified as generated, not authored.)

An important nuance for fairness: several of these existence lemmas are
tagged `research solved` — the mathematics is *known*, the formal proof is
not yet written. These are formalization holes, not epistemic ones. The
corpus's own category attributes record that intent. The point stands
unchanged: the trust reporting cannot see it, and neither can inspection of
the downstream theorem's source or type. These are the formal-statement
analogue of a transitive dependency no grep can find.

---

## 4. Computation trust: policy-invisible and closure-invisible

### 4.1 Compiler trust is invisible to policy in practice

890 declarations in the full corpus (234 authored) carry `Lean.ofReduceBool`
and `Lean.trustCompiler`; the two counts are exactly equal corpus-wide, which
corroborates at scale that the historical under-reporting bug in
`collectAxioms` (lean4#8840, now closed upstream: axioms referenced by the
types of other axioms were dropped, so `native_decide` could show
`ofReduceBool` without `trustCompiler`) is fixed at this toolchain — we also reran the issue's
minimal example and observed the complete set.

These are legitimate proofs under a larger trusted base. The failure is
institutional. The Palomar registry's published records carry an admission
field

```json
"permitted_axioms": ["propext", "Quot.sound", "Classical.choice"]
```

as carried in a registered record's `formalization` block (read from a
published Palomar record; the registry's public material states the same
three-axiom policy), under which every one of the 890 is inadmissible — while
Formal Conjectures records some of them, such as `erdos_730.variants.explicit_pairs` (proved by
`exact ⟨by decide, by native_decide⟩`), as proved textbook theorems. Both
positions are correct under their own policy. Neither system's records can
express the other's position. Section 7 develops the consequence. (Palomar holds no record of this
particular theorem; the contradiction is between Formal Conjectures' record
and Palomar's published policy, which is exactly how the repository's own
verification caveats state it.)

### 4.2 The closure can be silently wrong

lean4#7463 remains live at 4.27.0, and we reproduced it: an axiom
`cheating : False` attached to an `@[csimp]` replacement lemma proves
`one = 2` by `native_decide`, and `#print axioms` reports only
`[Lean.ofReduceBool, Lean.trustCompiler]` — the `False` axiom never appears.
The reported closure of a *false theorem* is indistinguishable from that of
any honest `native_decide` result.

A census of the flagship library bounds the exposure: all **193** `@[csimp]`
theorems registered in Mathlib's environment
(`Lean.Compiler.CSimp.ext.getState env |>.thmNames`) have axiom-clean
closures. The vector is live; the flagship library does not trip it. Both
facts belong in the accounting.

### 4.3 The closure can be silently uninformative

`erdos_730.variants.delta_ne_one` reports the maximally clean closure
`{propext, Quot.sound, Classical.choice}` while its proof asks the kernel to
evaluate `decide` over central binomial coefficients at n = 10003 and
n = 10005. Real computational trust — kernel reduction at nontrivial scale —
is exercised and adds no axiom. A policy that reasons only over axiom sets
rates this proof identical to a three-line rewrite.

### 4.4 The classes stack

All 3 authored declarations combining `sorryAx` with compiler trust are the
`OeisA87719` values: statements like `a 1 = 15`, proved by `decide +native`,
about the sequence defined through the sorried `a_exists`. A reader sees a
machine-checked concrete fact. The fact rests simultaneously on an unproved
existence claim and on trusting the compiler — and its own meaning is
conditional on the former.

---

## 5. The Effective Trust Closure

For a declaration `d` at source commit `c` under toolchain `t`, the Effective
Trust Closure is the tuple:

1. **statement_status** — `stated` | `statement_hole_direct` |
   `statement_hole_transitive`: walk the type's constant closure (types and
   values of reached constants) for `sorryAx`.
2. **axiom_closure** — the classical closure, unchanged.
3. **computation_trust** — `kernel_only` | `kernel_reduction_at_scale` |
   `compiler` (`ofReduceBool`/`trustCompiler` present).
4. **replacement_surface** — the `@[csimp]` lemmas reachable by native
   evaluation of `d`, with their own axiom closures: the lean4#7463 edge.
5. **environment** — the toolchain and dependency pins under which 1–4 were
   computed, without which none of them is a stable fact.

Components 1, 2, and 5, and the compiler arm of component 3, are implemented
and ran over the full corpus; the `kernel_reduction_at_scale` value is
presently assessed per case, not detected corpus-wide. Component 4 is
implemented at corpus level (the registered replacement set
and its closures); per-declaration reachability is future work, and we say
so. The implementation is a few hundred lines over `Lean.collectAxioms` and
`Expr.getUsedConstants`. The contribution is not the code. It is the
separation — and the evidence that the separated facts are what registries,
benchmarks, and successors already needed and could not see.

---

## 6. The census

### 6.1 Method

Enumerate the environment after importing every FC module that builds in the
pinned environment. The import list is the 1,061 of 1,090 modules (97.3%)
with prebuilt artifacts in the census environment; the 29 without — all under
`FormalConjectures/Arxiv/` — were not imported directly, though sampled
members compile cleanly, so the exclusion reflects the build configuration of
the environment, not build failures, and several of the 29 enter the
environment anyway through transitive imports (§9.5). For each
declaration originating in a FormalConjectures module, record the axiom
closure, the direct and transitive statement-hole flags, and compiler trust.
9,894 declarations.

### 6.2 Authored versus generated

Environments contain compiler-generated machinery — recursors, `noConfusion`,
constructor lemmas, equation lemmas, projections — that inherits trust
properties from its parent and must not inflate counts of *authored*
mathematics. We classify semantically, using the environment's own
predicates recorded per declaration by the census (`ConstantInfo` kind,
`Name.isInternalDetail`, `isAuxRecursor`, `isNoConfusion`,
`isProjectionFn`), plus disclosed residual name rules for equation, match,
and structure lemmas. One boundary case is disclosed: Lean's
`isInternalDetail` classifies a handful of authored declarations whose names
begin with an underscore-led component (a corpus idiom for numeral-leading
names) as internal; the raw census rows carry every flag, so any alternative
classification recomputes from the committed data. Both views:

| Population | All | Authored | Generated |
|---|---:|---:|---:|
| Declarations | 9,894 | 5,924 | 3,970 |
| `sorryAx` in closure | 2,885 | 2,862 | 23 |
| — statement holes | 864 | **850** | 14 |
| —— direct | 837 | 835 | 2 |
| —— hidden (transitive only) | 27 | **15** | 12 |
| — proof-only (stated, unproved) | 2,021 | 2,012 | 9 |
| Compiler trust | 890 | 231 | 659 |
| Mixed (`sorryAx` + compiler trust) | 3 | 3 | 0 |

The headline is robust to the classification: under a name-pattern filter,
an adversarial reviewer's corrected filter, and the semantic filter, the
statement-hole share of authored sorry-bearing declarations is 29.7% in all
three (850/2,862 semantic). The hidden class is 15 authored theorems across
six families, every one mechanism-verified in source (§3). Classification
matters most for compiler trust, where 659 of 890 carriers are generated
machinery; the authored surface is 231.

### 6.3 What the numbers mean, read honestly

The 835 direct statement holes are largely deliberate: `answer(sorry)` is the
corpus's intended encoding of "the answer is part of the question." The
finding is not that the corpus is careless. It is that a deliberate,
meaningful distinction made by the corpus's authors is **invisible to every
consumer of its trust reporting** — and that in the hidden class, the
authors' own source discipline cannot surface it either, because the hole
sits behind a definition in another part of the file or corpus.

---

## 7. The institutional consequence: one proof, a record and a policy in silent contradiction

At the pinned corpus commit, the three `Erdos730` declarations:

| Declaration | FC category | Statement | Computation trust | Admissible under Palomar's policy |
|---|---|---|---|---|
| `erdos_730` | research open | **hole (direct)** | — | no (`sorryAx`) |
| `variants.delta_ne_one` | research solved | stated | kernel reduction at scale | **yes** |
| `variants.explicit_pairs` | textbook, proved | stated | **compiler** | **no** |

Formal Conjectures records `explicit_pairs` as a proved theorem. Palomar's
published `permitted_axioms` would refuse the same proof. Both are correct
under their own rules; neither is aware of the other's position; nothing
anywhere relates them. This is not a defect in either registry. It is a fact
the shared accounting they both consume has no vocabulary for.

The consequence is live because environments move — including while this
paper was being written: Formal Conjectures' toolchain bump #4428 (to
v4.32.2, staging toward the 4.33 target its LeanEval export pins) merged on
2026-08-23, the day of this census. Computed from the effective closure —
not from anyone's memory of which proofs used `native_decide` — one event
yields five different correct consequences:

| Authority | Consequence of the toolchain bump | Why |
|---|---|---|
| erdosproblems.com | none | the mathematics did not change |
| FC / `delta_ne_one` | rebuild only | kernel proof; trust basis unaffected |
| FC / `explicit_pairs` | **re-verify the trust claim** | the proof trusts the compiler and the compiler changed |
| A registry record pinning this environment (Palomar-style mechanical report) | environment-stale | such records pin toolchain and Mathlib revision |
| Vela repository | scoped recheck; Standing unchanged until an authorized Decision | only an attributed Decision changes Standing |

A single status column cannot represent this. A dependency graph without
trust vocabulary computes the wrong blast radius: rebuilding `delta_ne_one`
is sufficient; rebuilding `explicit_pairs` is not.

Independent supporting measurement: a two-stage audit of the corpus's live
export seam (FC → LeanEval, 100 declarations, independently pinned toolchains:
source at 4.27, target workspaces carrying `lean-toolchain` v4.33.0 and
Mathlib `6f1ef4e5`) found 100/100 survive import and generation and 98/100 compile
at the target pins — with both failures being *faithful copies invalidated by
environment divergence*, exactly matching the seam's recorded known-failure
list. Handoff loss at a well-maintained boundary is rare, environmental in
origin, and — when it occurs — invisible to source inspection. Which is the
profile the effective closure exists to capture.

---

## 8. The live differential: one bump, measured the day it landed

Section 7 predicted, from recorded closures, that an environment move has
per-declaration consequences no single status can express. The move then
happened: #4428 merged at 22:24 UTC on 2026-08-23. We rebuilt the corpus at
the new pins (upstream `a3bc3141`, Lean 4.32.2) the same night, reran the
identical census, and joined the two on declaration name — to our knowledge
the first same-day, corpus-scale trust differential across a real toolchain
bump.

| Measure | Value |
|---|---:|
| Declarations before / after | 9,894 / 10,620 |
| Common declarations | 8,463 |
| Axiom closures changed | 451 — of which 282 in modules whose source is byte-identical |
| Statement meaning changed | **600 — of which 485 in byte-identical modules** |
| Compiler-trust vocabulary migrated | 236 common declarations (890 → 0 under the old name) |
| Conjectures proved in the interval | 1 (`OeisA113019.conjecture`) |
| Newly covered modules | 121 (1,342 of 2,157 appeared declarations) |

Three of these rows are findings in their own right.

**The trust vocabulary itself moved.** At 4.27 a `native_decide` proof
carries `Lean.ofReduceBool` and `Lean.trustCompiler`. At 4.32.2 the same
proof carries per-computation axioms of the form
`<decl>._native.native_decide.ax_1_1` — and **zero** declarations in the
rebuilt corpus carry `Lean.trustCompiler` at all (a corrected detector finds
1,188 carrying the new form). Every fixed-name policy over the axiom set
silently changed meaning: an allowlist still fails closed, but any tooling
that *detects* compiler trust by the old names — including this paper's own
census field, which we then corrected — reports zero. The accounting's
vocabulary is itself environment-relative, and nothing records the
migration.

**Statement identity did not survive the move.** 600 declarations flipped
from statement-hole to no-statement-hole — every one in the same direction.
The mechanism, verified directly: at 4.27 the corpus's `answer(sorry)` idiom
elaborates `erdos_942` to `sorry ↔ ∃ c > 0, …`; at 4.32.2 the **same
source line, through the same byte-identical macro,** elaborates to
`True ↔ ∃ c > 0, …`. The host language changed what the macro means, so the
same file states a different proposition under each toolchain — for 485
declarations whose modules are byte-identical across the pins. The standard
accounting flags none of it: the proofs carry `sorryAx` either way. This is
the paper's thesis in its sharpest form: not merely that trust facts are
invisible, but that *what is being claimed* is a function of the
environment, and only an accounting that pins the environment can say so.

**The differential also reads progress.** One conjecture
(`OeisA113019.conjecture`) lost its `sorryAx` in the interval — somebody
proved it — and 121 modules of new problems entered. A correction-aware
record derives the week's mathematical news mechanically, from the same
join.

The consequence table of §7 was hypothesis; this section is the event. The
five per-authority consequences it predicted are now concrete: kernel proofs
rebuilt silently, compiler-trusting proofs changed their trust *names*,
environment-pinned records went stale, and 600 statements changed meaning —
all invisible to any consumer of the per-declaration axiom set, all computed
here from two pinned censuses and a join.

---

## 9. Threats to validity

**9.1 Generated-declaration inflation.** Addressed by the semantic
authored/generated split (§6.2), validated adversarially in both directions,
with both views reported, all flags committed in the raw rows, and boundary
cases disclosed. The headline ratio is identical under three independent
classifications.

**9.2 "The statement holes are deliberate."** They are, mostly, and §6.3
says so. The claim is representational: the distinction the authors
deliberately encode is invisible to the reporting their consumers use. The
hidden class (§3) additionally defeats source inspection.

**9.3 "Known mathematics, not unknown."** Several sorried existence lemmas
are tagged `research solved`: formalization debt, not epistemic uncertainty.
The effective closure does not claim to distinguish those; it claims — and
this is the point — that today's reporting distinguishes *neither*.

**9.4 Single toolchain.** All measurements are at Lean 4.27.0 under the
corpus's own pin, with the environment recorded as part of the closure. The
lean4#8840 fix status and the lean4#7463 reproduction are toolchain-specific
facts and stated as such.

**9.5 Coverage.** 29 of 1,090 modules (2.7%), all under
`FormalConjectures/Arxiv/`, lacked prebuilt artifacts in the census
environment and were not imported directly. Sampled members compile cleanly,
so this is an artifact of the environment's build configuration, not of the
modules; several of the 29 nonetheless appear in the census through
transitive imports. The absence of the remainder cannot create the reported
classes, only hide additional instances; every ratio's denominator is the
censused population, stated explicitly.

**9.6 Self-verification.** The census and the demonstration repository were
produced and machine-verified by one producer plus a fresh-session verifier
sharing a model family. The acceptance record says exactly that
(`does_not_establish: full independence …`). The mitigations are mechanical:
every number re-derives from committed scripts against a public corpus at a
pinned commit, and the discriminator was validated against positive and
negative controls before use.

**9.7 Frequency versus structure.** This paper measures representational
incompleteness, not exploitation. The one known smuggling vector is untripped
in Mathlib's 193 registered replacements. The institutional contradiction of
§7, however, is not hypothetical: the records carrying the two positions both
exist today.

---

## 10. The closure as scientific state

A fact about trust is useful only if it survives as state: attributable,
bounded, environment-pinned, and correction-aware. We recorded the §7 case as
a bounded claim in a signed Vela repository and ran its full lifecycle:

- The claim binds the exact source commit, file digest, toolchain, and
  per-declaration closures, with explicit caveats, verbatim: "Does not establish that Erdos 730 is
  solved; the problem is open"; "… that either authority is wrong; both are
  correct under their own policy"; "Does not establish acceptance or
  endorsement by Formal Conjectures, Palomar, or erdosproblems.com."
- The producer's own verification — same actor, same script — was retained
  with `independent_of_producer: false`, and the acceptance gate **refused
  it**: `protocol_gate: blocked`, blocker
  `missing_independent_passing_verification`, accepted state unchanged.
- A verifier in a fresh session, writing its own Lean file and using two
  mechanisms (`#print axioms` and `Lean.collectAxioms`), reproduced all three
  closures set-exactly. Only then did the gate read `satisfied`, and an
  attributed Decision accepted the claim as a bounded computational
  observation about what each system's records say.
- The repository replays offline — signatures, roots, canonical bytes — with
  no account, server, or key.

Two of this paper's observations (§4.3's kernel-reduction case and the
statement-hole reading of `erdos_730`) were surfaced by the independent
verifier, not the producer, and entered as retained verification artifacts.
What is durable is the producer verification's declared non-independence and
scope, from which the gate's refusal re-derives on replay; the refusal is
recomputed policy over retained state, not a stored flag — which is the
property that separates a trust *record* from a trust *scanner*.

---

## 11. Reproducing

Corpus: `williamjblair/formal-conjectures` @
`e51535ae2caeab6c7493450a5d86a5a8651fa82d` (Lean sources byte-identical to
upstream google-deepmind/formal-conjectures @ `9f5ee773`), toolchain
`leanprover/lean4:v4.27.0`, prebuilt; every script below runs with
`lake env lean <file>` from the checkout root, no `lake build` required.

Scripts (in `vela-evals/adapters/lean-axiom-audit/`):

- `trust-closure-census.lean` — the full census with semantic classification
  flags; writes 9,894 rows, committed as
  `fc-trust-closure-v2.e51535ae.jsonl` (the earlier flagless run is retained
  as `fc-trust-closure.e51535ae.jsonl`).
- `stmt-hole-probe.lean` — discriminator validation, positive and negative
  controls.
- `erdos730.lean` — the three Erdős 730 closures against Palomar's policy.
- `csimp-census.lean` — the 193 Mathlib replacement lemmas.
- `csimp7463.lean` — the live smuggling reproduction.
- `closure.lean`, `nativecheck.lean` — the lean4#8840 fix-status checks.
- The authored/generated filter and all aggregate tables: the analysis
  snippets committed alongside the raw JSONL.

Seam audit: `formal-conjectures/comparator` (branch `comparator-workspaces`),
whole-set import via `make_comparator_workspace.py --set FC100OpenSet1
--verify`, target compile via `compile_fc100_target.py`; result 98/100 with
failures exactly matching `known_failures.toml`.

The signed repository: claim
`vcl_6a5b08afce4720db40e53c44a0fc85ca1a64eaacb9e583130dba9d568331fd71`,
repository root
`sha256:bdd0f36e7591f702c7d4f9ec354c8010f38e6fa5949cb6e6d69b37cd3d6e4a85`,
inspected with `vela status` / `vela why` (v0.977.4, binary digest
`06f912d1…` verified against its signed release manifest).

---

## 12. What this does not establish

- That Formal Conjectures, Palomar, Mathlib, or Lean are defective. Every
  behavior reported is by design or documented upstream; the claim is about
  what the shared accounting can express.
- That any Erdős problem is solved, or any proof unsound.
- Frequency of harm, adoption, endorsement, or review by any named project.
- General claims beyond this corpus, this toolchain, and these registries'
  published records — each of which is pinned, named, and re-derivable.
