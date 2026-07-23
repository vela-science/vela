import Mathlib

/-!
# Polynomial reconstruction uniqueness used by threshold sharing

For polynomials of degree `< k`, agreement at at least `k` distinct field
points implies equality. The corollary gives equality at zero. These are
reconstruction-uniqueness lemmas for the definitions below; they do not prove a
complete Shamir implementation, privacy, robustness, repository transfer, or
Vela standing.
-/

namespace Vela.TransferMDSToSecretSharing

open Polynomial

variable {F : Type*} [Field F] [DecidableEq F]

/-- Two degree-`< k` polynomials agreeing on at least `k` distinct points are
    equal. -/
theorem shares_determine_polynomial {k : ℕ} (p q : F[X])
    (hp : p.degree < (k : ℕ)) (hq : q.degree < (k : ℕ))
    (pts : Finset F) (hcard : k ≤ pts.card)
    (hagree : ∀ x ∈ pts, p.eval x = q.eval x) :
    p = q := by
  by_contra hne
  have hd : p - q ≠ 0 := sub_ne_zero.mpr hne
  have hdeg : (p - q).degree < (k : ℕ) := lt_of_le_of_lt (degree_sub_le p q) (max_lt hp hq)
  have hndeg : (p - q).natDegree < k := (natDegree_lt_iff_degree_lt hd).mpr hdeg
  have hsub : pts ⊆ (p - q).roots.toFinset := by
    intro x hx
    rw [Multiset.mem_toFinset, mem_roots hd]
    show (p - q).eval x = 0
    rw [eval_sub, hagree x hx, sub_self]
  have h1 : pts.card ≤ (p - q).roots.toFinset.card := Finset.card_le_card hsub
  have h2 : (p - q).roots.toFinset.card ≤ Multiset.card (p - q).roots := (p - q).roots.toFinset_card_le
  have h3 : Multiset.card (p - q).roots ≤ (p - q).natDegree := card_roots' (p - q)
  omega

/-- **Unique secret.** Under the same hypotheses, the recovered secret `p(0)` is unique. -/
theorem secret_recovered {k : ℕ} (p q : F[X])
    (hp : p.degree < (k : ℕ)) (hq : q.degree < (k : ℕ))
    (pts : Finset F) (hcard : k ≤ pts.card)
    (hagree : ∀ x ∈ pts, p.eval x = q.eval x) :
    p.eval 0 = q.eval 0 := by
  rw [shares_determine_polynomial p q hp hq pts hcard hagree]

end Vela.TransferMDSToSecretSharing
