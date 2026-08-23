import Mathlib

/-!
# Erdős Problem #730

*Are there infinitely many pairs of integers `n < m` such that `(2n choose n)` and
`(2m choose m)` have the same set of prime divisors?*

Erdős, Graham, Ruzsa and Straus [EGRS75] posed the question and believed there was
"no doubt" the answer is yes; `(87, 88)` and `(607, 608)` are such pairs, and the
parameters `n` admitting some `m > n` are OEIS A129515. The problem is
[erdosproblems.com/730](https://www.erdosproblems.com/730), and the set in the theorem below is
copied verbatim from its formal statement in Google DeepMind's Formal Conjectures
(`FormalConjectures/ErdosProblems/730.lean`), so that a reader auditing this file
is auditing the same object the problem registry states.

The theorem `erdos_730_infinite` asserts the affirmative answer: `S` is infinite. The
proof (in `Solution`) formalises the argument posted informally by Liam Price on the
problem's discussion page on 24 June 2026 — equality of prime supports is converted to
a base-`p` digit condition by Kummer's theorem, an explicit quadratic family of
consecutive pairs `(n, n + 1)` is constructed, a fixed-depth Fourier estimate shows the
digit obstruction has density `≈ 4^{-r}` on blocks of length `p^r`, and the finitely
many obstruction prime ranges are handled by case analysis — and in fact produces
infinitely many *consecutive* pairs.
-/

namespace Palomar.Erdos730

/-- **Erdős #730**, answered affirmatively: there are infinitely many pairs `n < m`
with `(2n choose n)` and `(2m choose m)` sharing their set of prime divisors.

The set written out here is, character for character, the set `S` of
`FormalConjectures/ErdosProblems/730.lean`; it is inlined rather than named so that this
statement depends on Mathlib alone and a reader audits exactly one declaration. -/
theorem erdos_730_infinite :
    {(n, m) : ℕ × ℕ | n < m ∧ n.centralBinom.primeFactors = m.centralBinom.primeFactors}.Infinite := by
  sorry

end Palomar.Erdos730
