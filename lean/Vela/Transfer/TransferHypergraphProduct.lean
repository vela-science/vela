import Mathlib

/-!
# Hypergraph-product CSS commutation precondition

The construction behind modern good quantum-LDPC codes (Tillich–Zémor), proven sound *in general*. From any
classical parity-check matrix `H` over `GF(2)` form the block matrices

  `Hx = [ H ⊗ I | I ⊗ Hᵀ ]`,   `Hz = [ I ⊗ H | Hᵀ ⊗ I ]`.

We prove `Hx · Hzᵀ = 0` for every `H`. Together with
`Vela.TransferClassicalToCSS.css_commute`, this establishes pairwise
commutation for the generated operators. It does not prove rank, encoded
dimension, distance, or executable-verifier refinement. The proof is the
Kronecker mixed-product identity plus characteristic two:
`Hx Hzᵀ = H⊗Hᵀ + H⊗Hᵀ = 0`.
-/

namespace Vela.TransferHypergraphProduct

open Matrix
open scoped Kronecker

variable {m n : ℕ} (H : Matrix (Fin m) (Fin n) (ZMod 2))

/-- X-check matrix of the hypergraph product: `[ H ⊗ Iₙ | Iₘ ⊗ Hᵀ ]`. -/
def Hx : Matrix (Fin m × Fin n) ((Fin n × Fin n) ⊕ (Fin m × Fin m)) (ZMod 2) :=
  fromCols (H ⊗ₖ (1 : Matrix (Fin n) (Fin n) (ZMod 2)))
           ((1 : Matrix (Fin m) (Fin m) (ZMod 2)) ⊗ₖ Hᵀ)

/-- Z-check matrix of the hypergraph product: `[ Iₙ ⊗ H | Hᵀ ⊗ Iₘ ]`. -/
def Hz : Matrix (Fin n × Fin m) ((Fin n × Fin n) ⊕ (Fin m × Fin m)) (ZMod 2) :=
  fromCols ((1 : Matrix (Fin n) (Fin n) (ZMod 2)) ⊗ₖ H)
           (Hᵀ ⊗ₖ (1 : Matrix (Fin m) (Fin m) (ZMod 2)))

/-- The matrices satisfy the stated CSS commutation precondition over `GF(2)`
    for every `H`; no parameter or distance theorem is included. -/
theorem hgp_css_precondition : (Hx H) * (Hz H)ᵀ = 0 := by
  have h2 : ∀ x : ZMod 2, x + x = 0 := by decide
  unfold Hx Hz
  rw [transpose_fromCols, fromCols_mul_fromRows]
  -- resolve the transposes of the Kronecker blocks, keeping Kronecker form
  simp only [← kroneckerMap_transpose, transpose_one, transpose_transpose]
  -- combine each product of Kroneckers: (A⊗A')(B⊗B') = (AB)⊗(A'B')  [mul_kronecker_mul, reversed]
  rw [← mul_kronecker_mul, ← mul_kronecker_mul]
  simp only [Matrix.mul_one, Matrix.one_mul]
  -- goal is now  H⊗Hᵀ + H⊗Hᵀ = 0; characteristic two finishes it entrywise
  ext i j
  simp only [Matrix.add_apply, Matrix.zero_apply]
  exact h2 _

end Vela.TransferHypergraphProduct
