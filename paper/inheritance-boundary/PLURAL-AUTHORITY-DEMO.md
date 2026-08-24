# Plural authority: a demonstration design

**Status: design. Nothing built yet.**

The claim this demonstrates is not that Vela is faster or more accurate. It is
that Vela represents a state that other tools structurally cannot hold, and
computes a consequence that other tools structurally cannot compute.

No control arm. No baseline. No model has to fail. The artifact is the
evidence, the way a running chain was the evidence for Bitcoin and two
machines exchanging hypertext was the evidence for HTTP.

## The object

Erdős Problem 730: are there infinitely many pairs `n < m` where the central
binomials `C(2n,n)` and `C(2m,m)` have the same set of prime divisors?

It is chosen because four independent systems already hold a position on it,
and their positions already disagree in a way that is *correct*.

## The state that already exists

| Authority | Position | Basis |
| --- | --- | --- |
| Erdős Problems (erdosproblems.com/730) | open | the infinitude question is unresolved |
| Formal Conjectures | `erdos_730`: `research open`, `answer(sorry)` | the main statement is not formalized as proved |
| Formal Conjectures | `erdos_730.variants.delta_ne_one`: `research solved` | proved in Lean; found by AlphaProof |
| Formal Conjectures | `erdos_730.variants.explicit_pairs`: `textbook`, proved | proved in Lean via `decide` and `native_decide` |
| Palomar | `registered`, mechanical `pass` | comparator and nanoda at pinned toolchain, environment recorded |
| Vela Math Repository | accepted bounded Claim | attributed Decision, with what the check did not establish attached |

Every row is correct. None contradicts another. A relational schema with one
`status` column destroys this; a citation graph never had it; Git tracks the
files but not what any status depends upon.

That is the first half of the demonstration, and it requires only assembling
what is already true.

## The asymmetry that makes it sharp

The two proved variants of the same problem **do not rest on the same
foundation**, and no ordinary record says so.

- `delta_ne_one` is proved by `norm_num` and `decide`. Kernel only.
- `explicit_pairs` is proved partly by `native_decide`. Its axiom closure
  therefore contains `Lean.ofReduceBool` and `Lean.trustCompiler`.

"There is a formal proof" is true of both and is the wrong amount of
information. One is checked by the Lean kernel; the other additionally trusts
the compiler. Verified 2026-08-23: on Lean 4.27 a `native_decide` proof
reports both `Lean.ofReduceBool` and `Lean.trustCompiler`, and across the
whole Formal Conjectures corpus 890 declarations carry that pair.

Vela's Result and Check primitives carry the axiom closure and the explicit
"does not establish" field. That is where this asymmetry survives a handoff.

## The correction, and why its consequence differs per authority

Formal Conjectures is planning a toolchain bump to Lean 4.33 (staged in
#4428). This is a real, scheduled, already-referenced change: two entries in
`comparator/known_failures.toml` are recorded as retired by it, verified by
kim-em on 2026-08-21.

When the environment moves, the correct consequence is **different for each
authority**, and each is computable:

| Authority | Consequence | Why |
| --- | --- | --- |
| Erdős Problems | none | the mathematics did not change |
| FC `delta_ne_one` | recheck build only | kernel proof; trust basis unchanged |
| FC `explicit_pairs` | **re-verify the trust claim** | the compiler changed and the proof trusts the compiler |
| Palomar | record becomes environment-stale | its mechanical report pins `lean_toolchain` and `mathlib_cache` |
| Vela Claim | recheck scoped to the affected assumption; Standing unchanged until an authorized Decision | only a Decision moves Standing |

One event. Five different correct answers. Computed from recorded dependency,
not from a human remembering which results were `native_decide`.

**This is the part no other tool can do.** Not "does it faster". Cannot.

## What to build

1. **Assemble the state.** Bind the four authorities' current positions on
   Erdős 730 as exact references: erdosproblems.com/730, the three FC
   declarations at a pinned FC commit, the Palomar record and its mechanical
   report, the Vela Claim in the public Math Repository.
2. **Record the trust asymmetry.** Compute the axiom closure of each proved FC
   declaration and attach it, so `explicit_pairs` carries `trustCompiler` and
   `delta_ne_one` does not. The audit script already exists.
3. **Apply the correction.** Take the 4.33 toolchain bump as the source
   change. Compute the impact closure across the four authorities.
4. **Publish it replayable.** `vela status` and `vela why` already replay a
   repository offline with no account, no daemon, no key. A reader clones,
   replays, and sees the four positions and the propagated consequence
   without trusting any server.

## What it must not claim

- Not that Erdős 730 is solved. It is open.
- Not that any authority's status is wrong. All four are correct.
- Not that `native_decide` results are unsound. They are proofs under a
  larger trust base, correctly reported by Lean 4.27.
- Not adoption, productivity, or scientific lift.
- Not that Palomar, FC, or Erdős Problems endorse the representation.

The claim is exactly: this state exists, it is not expressible in the tools
these systems use, and its correction consequence is computable here.

## Falsification

The demonstration fails, and should be abandoned, if any of these hold:

- the four positions collapse to one status without losing information, so
  plural authority is decoration;
- the correction consequence is identical for every authority, so propagation
  is trivial;
- an ordinary tool already expresses it — if a Git branch, a registry field,
  or a citation graph carries the same distinctions, Vela is redundant here;
- the trust asymmetry turns out to be recorded adequately by existing FC or
  Palomar metadata, making the Check primitive unnecessary for this case.

Each is checkable before building the full artifact. Check them first.

## Why this shape

Bitcoin did not run a controlled trial against banks. It solved double-spend
without a trusted third party and published a chain anyone could run. HTTP did
not measure task completion. It connected two machines that could not talk.

Neither waited for external validation, and neither needed a delta over a
baseline. They demonstrated a capability that did not previously exist, in a
form anyone could verify by running it.

One disanalogy is worth stating plainly: double-spend was a famous problem, so
the demonstration landed immediately. Plural authority over scientific state is
not famous. So this artifact has to make a reader feel the problem before it
shows the answer, which is what the cross-system drift findings are for. That
is a writing constraint, not a validation gate.
