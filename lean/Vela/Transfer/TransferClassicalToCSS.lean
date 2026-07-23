import Mathlib

/-!
# Pairwise commutation for CSS-style stabilizer generators

Given binary matrices `Hx` and `Hz` satisfying the stated orthogonality
premise, this file proves that the resulting X- and Z-type Pauli generators
commute pairwise. It does not prove matrix rank, encoded dimension, code
distance, independence of generators, refinement of `scripts/verify_qec.py`,
or the existence of a quantum `[[n,k,d]]` code with claimed parameters.
-/

namespace Vela.TransferClassicalToCSS

open Finset

variable {n : ℕ}

/-- A Pauli operator on `n` qubits in symplectic `GF(2)` form: an `X`-part and a `Z`-part. -/
structure Pauli (n : ℕ) where
  x : Fin n → ZMod 2
  z : Fin n → ZMod 2

/-- Symplectic inner product. Two Paulis commute iff it is `0`. -/
def symplectic (p q : Pauli n) : ZMod 2 := ∑ a, (p.x a * q.z a + p.z a * q.x a)

/-- X-type stabilizer from a row of `Hx` (zero Z-part). -/
def Xstab {rx : ℕ} (Hx : Fin rx → Fin n → ZMod 2) (i : Fin rx) : Pauli n :=
  ⟨Hx i, fun _ => 0⟩

/-- Z-type stabilizer from a row of `Hz` (zero X-part). -/
def Zstab {rz : ℕ} (Hz : Fin rz → Fin n → ZMod 2) (j : Fin rz) : Pauli n :=
  ⟨fun _ => 0, Hz j⟩

/-- The full CSS stabilizer family, indexed by `Fin rx ⊕ Fin rz`. -/
def stab {rx rz : ℕ} (Hx : Fin rx → Fin n → ZMod 2) (Hz : Fin rz → Fin n → ZMod 2) :
    (Fin rx ⊕ Fin rz) → Pauli n
  | Sum.inl i => Xstab Hx i
  | Sum.inr j => Zstab Hz j

/-- If `Hx · Hzᵀ = 0` over `GF(2)`, the generators defined above commute
    pairwise. This is one necessary CSS-code condition, not a full code
    certificate. -/
theorem css_commute {rx rz : ℕ}
    (Hx : Fin rx → Fin n → ZMod 2) (Hz : Fin rz → Fin n → ZMod 2)
    (hortho : ∀ i j, ∑ a, Hx i a * Hz j a = 0) :
    ∀ s t : Fin rx ⊕ Fin rz, symplectic (stab Hx Hz s) (stab Hx Hz t) = 0 := by
  intro s t
  cases s with
  | inl i =>
    cases t with
    | inl j => simp [stab, Xstab, symplectic]
    | inr j =>
      simp only [stab, Xstab, Zstab, symplectic, mul_zero, add_zero]
      exact hortho i j
  | inr i =>
    cases t with
    | inl j =>
      simp only [stab, Xstab, Zstab, symplectic, mul_zero, zero_add]
      rw [show (∑ a, Hz i a * Hx j a) = ∑ a, Hx j a * Hz i a from
        Finset.sum_congr rfl (fun a _ => mul_comm _ _)]
      exact hortho j i
    | inr j => simp [stab, Zstab, symplectic]

/-- Self-commutation is automatic in this representation (over `GF(2)`, `2 = 0`), recorded for clarity. -/
theorem symplectic_self (p : Pauli n) : symplectic p p = 0 := by
  unfold symplectic
  apply Finset.sum_eq_zero
  intro a _
  have h : p.x a * p.z a + p.z a * p.x a = 2 * (p.x a * p.z a) := by ring
  rw [h, show (2 : ZMod 2) = 0 by decide, zero_mul]

end Vela.TransferClassicalToCSS
