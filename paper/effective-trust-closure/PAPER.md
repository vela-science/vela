# The Effective Trust Closure

What a formal mathematical result actually rests on, why the standard
accounting cannot say, and a census of the flagship AI-conjecture corpus.

**Status: draft. Numbers are computed and reproducible; framing is not
frozen. Nothing here has been reviewed by the maintainers of any corpus or
registry it discusses.**

## Abstract

Formal mathematics has one standard trust accounting: the axiom closure, as
reported by `#print axioms` and consumed by admission policies, benchmarks,
and downstream formalization efforts. We show that this accounting conflates
at least four distinct trust states, each verified concretely on Lean 4.27.0,
and we define the Effective Trust Closure that separates them.

A census of all 9,894 built declarations in google-deepmind/formal-conjectures
(commit `e51535ae`) finds that of the 2,885 declarations whose closure
contains `sorryAx`, **864 (30%) are statement holes**: the statement itself,
not merely the proof, is incomplete. 837 of these are directly visible in the
declaration's type; **27 are hidden** — the statement reads as fully stated
but transitively rests on a `sorry`-defined dependency. A further 890
declarations (9.0% of the corpus) rest on compiler trust, and 3 declarations
combine both: concrete numerical facts whose proofs invoke native evaluation
about a sequence whose defining existence proof is `sorry`.

No existing tool distinguishes any of these classes. We show the conflation
already has an institutional consequence: two live registries hold
policy-contradictory positions on the same theorem without either being wrong
or either being able to see it. Finally, we demonstrate the closure operating
as durable scientific state: recorded as a bounded claim in a signed
repository whose acceptance gate refused the producer's own passing check
until an independent recomputation existed.

## 1. Four trust states one report conflates

`#print axioms` answers one question: which axioms does this constant's
elaborated term transitively use. Every failure below is a fact that question
cannot express.

### 1.1 Unproved is not the same as unstated

Formal Conjectures deliberately formalizes open questions whose *answer* is
part of the unknown: `theorem erdos_730 : answer(sorry) ↔ S.Infinite`. Here
`sorryAx` enters through the **type**. A fully stated conjecture with a
`sorry` proof — `erdos_23.variants.n1` — produces the identical
`#print axioms` line, with `sorryAx` entering only through the **value**.

The distinction is material to every consumer. An unproved statement can be
proved. A statement hole cannot; it must first be instantiated. A prover
benchmarked on the corpus is, for 30% of its sorry-bearing targets, being
handed a hole presented as a statement.

Detection is mechanical once named: check whether the declaration's type
(directly, or transitively through its dependencies) uses `sorryAx`. We
verified the discriminator against both classes before running it at scale.

### 1.2 Hidden statement holes

The sharp class is the 27 declarations whose type does **not** mention
`sorryAx` but transitively reaches it. Two mechanisms, both confirmed in
source:

- **Definition through unproved existence.** `OeisA87719.a_exists` (the
  sequence is well-defined) is proved by `sorry`. The sequence is then
  defined as `a (n) := Nat.find (a_exists n)`. Every theorem about `a`
  reads as fully stated and is a statement about a partial object.
- **Choice from unproved existence.** In `Wikipedia/MovingSofa.lean`,
  `ABφθSpec.existsUnique := sorry`, and Gerver's constants are defined as
  `def A : ℝ := ABφθSpec.existsUnique.choose.1`. Downstream theorems about
  the Gerver sofa are statements about numbers chosen from a hole.

These are the formal-statement analogue of a transitive dependency no grep
can find. Inspecting the theorem's source shows nothing; inspecting its
printed axiom closure shows `sorryAx` but attributes it, misleadingly, to an
incomplete *proof*.

### 1.3 Compiler trust is policy-invisible in practice

890 declarations (9.0%) carry `Lean.ofReduceBool` and `Lean.trustCompiler` —
the corpus-wide counts are exactly equal, corroborating that the historical
under-reporting bug (lean4#8840) is fixed at this toolchain. These are
legitimate proofs under a larger trusted base. The failure is institutional:
admission policies reason over the closure as a set. The Palomar registry's
published records carry `permitted_axioms: [propext, Quot.sound,
Classical.choice]`. Under that policy every one of the 890 is inadmissible,
while Formal Conjectures records some of them — `erdos_730.variants.
explicit_pairs`, proved partly by `native_decide` — as proved textbook
theorems. Both positions are correct under their own policy. No record in
either system relates them. Section 3 develops this case.

### 1.4 The closure can be silently wrong, and silently uninformative

Two boundary results, both reproduced on Lean 4.27.0:

- **Wrong:** lean4#7463 remains live. An axiom `cheating : False` smuggled
  through an `@[csimp]` replacement lemma proves `one = 2` by
  `native_decide`, and `#print axioms` reports only
  `[Lean.ofReduceBool, Lean.trustCompiler]` — the `False` axiom never
  appears. (A census of all 193 `@[csimp]` theorems in Mathlib found every
  one axiom-clean; the vector is live, the flagship library does not trip
  it.)
- **Uninformative:** `erdos_730.variants.delta_ne_one` reports the maximally
  clean closure `{propext, Quot.sound, Classical.choice}` while its proof
  has the kernel evaluate `decide` over central binomial coefficients at
  n = 10003 and 10005. Real computational trust — kernel reduction at
  nontrivial scale — is exercised and adds no axiom.

### 1.5 Mixed trust

The classes stack. All 3 corpus declarations combining `sorryAx` with
compiler trust are the `OeisA87719` values `a_1`, `a_2`, `a_3`: statements
like `a 1 = 15`, proved by `decide +native`, about a sequence defined through
a sorried existence proof. A reader sees a machine-checked concrete fact. The
fact rests simultaneously on an unproved existence claim and on trusting the
compiler, and its own meaning is conditional on the former.

## 2. The Effective Trust Closure

For a declaration `d` at source commit `c` under toolchain `t`, the Effective
Trust Closure is the tuple:

1. **statement_status** — `stated` | `statement_hole_direct` |
   `statement_hole_transitive`, by walking the type's constant closure for
   `sorryAx`;
2. **axiom_closure** — the classical closure, kept as-is;
3. **computation_trust** — `kernel_only` | `kernel_reduction_at_scale` |
   `compiler` (`ofReduceBool`/`trustCompiler` present);
4. **replacement_surface** — the `@[csimp]` lemmas reachable by native
   evaluation of `d`, with their own axiom closures (the lean4#7463 edge);
5. **environment** — toolchain and dependency pins under which 1–4 were
   computed, without which none of them is a stable fact.

Components 1, 2, 3 and 5 are implemented and ran over the full corpus;
component 4 is implemented as a corpus-level census (per-declaration
reachability is future work). Everything is a few hundred lines over
`Lean.collectAxioms` and `Expr.getUsedConstants`; the contribution is not the
code but the separation — and the demonstration that the separated facts are
what registries, benchmarks, and successors actually need.

## 3. The institutional consequence: one theorem, two registries, silent contradiction

At Formal Conjectures commit `e51535ae`, the three `Erdos730` declarations
have distinct effective closures:

| Declaration | FC category | Statement | Computation trust | Admissible under Palomar's `permitted_axioms` |
|---|---|---|---|---|
| `erdos_730` | research open | **hole (direct)** | — | no (`sorryAx`) |
| `variants.delta_ne_one` | research solved | stated | kernel reduction at scale | **yes** |
| `variants.explicit_pairs` | textbook, proved | stated | **compiler** | **no** |

Formal Conjectures records `explicit_pairs` as a proved theorem. Palomar's
policy would refuse it. Both are correct under their own rules. Neither
system's records can express the other's position, and nothing anywhere
relates them. This is not a defect in either registry: it is a fact that the
shared accounting they both consume — the axiom closure — has no vocabulary
for.

The consequence is live because the environment moves. Formal Conjectures
stages a toolchain bump to Lean 4.33 (#4428). Computed from the effective
closure, one event yields five different correct consequences: nothing for
the problem's source of record; rebuild only for the kernel proof;
re-verification of the trust claim for the compiler-trusting proof;
environment-staleness for the registry record that pins the toolchain; a
scoped recheck with unchanged standing for the state layer. A single status
column cannot represent this; a dependency graph without trust vocabulary
computes the wrong blast radius (rebuilding `delta_ne_one` is sufficient;
rebuilding `explicit_pairs` is not).

## 4. The census

All built modules of google-deepmind/formal-conjectures at `e51535ae`
(1,061 of 1,090 modules, 97.3%; the remainder do not build in the pinned
environment), under Lean 4.27.0:

| Population | Count | Share |
|---|---:|---:|
| Declarations censused | 9,894 | — |
| `sorryAx` in closure | 2,885 | 29.2% |
| — statement holes, direct | 837 | 29.0% of sorry-bearing |
| — statement holes, hidden (transitive only) | 27 | 0.9% |
| — proof-only holes (stated, unproved) | 2,021 | 70.1% |
| Compiler trust | 890 | 9.0% |
| Mixed (`sorryAx` + compiler trust) | 3 | — |

Reading the numbers honestly: the 837 direct statement holes are largely
deliberate — `answer(sorry)` is Formal Conjectures' intended encoding of
"the answer is part of the question." The finding is not that the corpus is
careless. It is that a deliberate, meaningful distinction made by the
corpus's authors is **invisible to every consumer of its trust reporting**,
and that in the hidden class the authors' own type discipline cannot surface
it either.

## 5. The closure as scientific state

A fact about trust is only useful if it survives as state: attributable,
bounded, and correction-aware. We recorded the Section 3 case as a bounded
claim in a signed Vela repository and ran its full lifecycle:

- the claim binds the exact source commit, file digest, toolchain, and
  per-declaration closures;
- the producer's own verification — same actor, same script — was retained
  with `independent_of_producer: false`, and the acceptance gate **refused
  it**: `protocol_gate: blocked`, `missing_independent_passing_verification`,
  with accepted state unchanged;
- a verifier in a fresh session, writing its own Lean file and using two
  mechanisms (`#print axioms` and `Lean.collectAxioms`), reproduced all
  three closures set-exactly; only then did the gate read `satisfied`;
- the attributed Decision accepted the claim *as a bounded computational
  observation about what each system's records say* — explicitly not as a
  statement about Erdős 730, and not as endorsement by any named registry;
- the repository replays offline: signatures, roots, canonical bytes.

The independent verifier also surfaced Section 1.4's second half — the
kernel-reduction observation — and the statement-hole reading of
`erdos_730`, both of which entered this paper as retained verification
artifacts rather than as the producer's claims. A green check from the
producer moved nothing; the record of *why* it moved nothing is part of the
state.

## 6. Related boundaries

- lean4#8840 (`collectAxioms` missing axioms referenced by axiom types) is
  fixed at 4.27: the documented repro returns the full set, and the corpus
  count `ofReduceBool` = `trustCompiler` = 890 corroborates at scale.
- lean4#7463 (`@[csimp]` smuggling) is live at 4.27 and reproduced; Mathlib's
  193 csimp theorems are clean.
- Prior work on proof assumptions distinguishes hypothesis-parameters from
  axiomatized lemmas; the statement-hole classes here are orthogonal to both
  and, to our knowledge, unmeasured before this census.

## 7. What this does not establish

- That Formal Conjectures, Palomar, Mathlib, or Lean are defective. Every
  behavior reported is by design or documented; the claim is about what the
  shared accounting can express.
- That any Erdős problem is solved or any proof unsound.
- Adoption, endorsement, or review by any named project. The census and the
  repository are the work of one producer and one fresh-session verifier
  sharing a model family; the verification records say so.
- Frequency of harm. The census measures representational incompleteness,
  not exploitation: the one known smuggling vector is untripped in the
  flagship library.

## 8. Reproducing

- Census and probes: `vela-evals/adapters/lean-axiom-audit/`
  (`trust-closure-census.lean`, `stmt-hole-probe.lean`, `csimp-census.lean`,
  `erdos730.lean`, `closure.lean`, `nativecheck.lean`, `csimp7463.lean`),
  run with `lake env lean` in the pinned checkout. No `lake build` required.
- Corpus: google-deepmind/formal-conjectures `e51535ae`, Lean 4.27.0.
- Raw census output: `fc-trust-closure.jsonl`, 9,894 rows.
- The signed repository: claim
  `vcl_6a5b08afce4720db40e53c44a0fc85ca1a64eaacb9e583130dba9d568331fd71`,
  repository root
  `sha256:bdd0f36e7591f702c7d4f9ec354c8010f38e6fa5949cb6e6d69b37cd3d6e4a85`,
  replayable with `vela status` / `vela why` and no server.
