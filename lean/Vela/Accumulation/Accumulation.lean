/-!
# Historical trusted-fold invariant model

This file models a trusted computation that carries a running `state` and one
Boolean invariant `ok`. Folding a delta keeps `ok = true` only if the supplied
verifier passes and the numeric frontier strictly improves. `accumulate_sound`
proves an invariant of that trusted fold.

The Boolean is not a cryptographic proof binding an external checker to the
history, and the model does not provide succinct verification, an accumulator
scheme, IVC, PCD, or a light-client protocol. A real construction would need to
prove the actual Vela transition relation and bind every step and root. The
historical declarations remain for research reproduction and are not imported
by the active theorem aggregate.

The model has one supplied verifier and a state function whose in-memory size
is not analyzed. It assumes the trusted fold was executed as defined. It makes
no claim about external proof size, consensus, authority, credit, or adoption.
-/

namespace Vela.Accumulation

/-- Frontier identifiers, quality levels, and the verified state (best level per frontier). -/
abbrev Frontier := Nat
abbrev Level := Nat
abbrev State := Frontier → Level

/-- A contribution: raise `frontier` to `level`, backed by `witness`. -/
structure Delta where
  frontier : Frontier
  level    : Level
  witness  : Nat

/-- The trusted fold state. No memory-size or cryptographic-proof bound is
    established by this structure. -/
structure Acc where
  state : State
  ok    : Bool

/-- The genesis accumulator: nothing verified, integrity intact. -/
def init : Acc := { state := fun _ => 0, ok := true }

/-- Fold one delta into the accumulator under the FROZEN verifier `verify`. The integrity bit stays
    true only if this delta passed the verifier AND strictly improved its frontier; otherwise the bit
    is cleared (and stays cleared — see `fold_preserves_false`). A rejected delta never changes state. -/
def fold (verify : Delta → Bool) (a : Acc) (d : Delta) : Acc :=
  if verify d = true ∧ d.level > a.state d.frontier then
    { state := fun f => if f = d.frontier then d.level else a.state f, ok := a.ok }
  else
    { a with ok := false }

/-- Fold a finite history into the trusted model state. -/
def accumulate (verify : Delta → Bool) (ds : List Delta) : Acc :=
  ds.foldl (fold verify) init

/-- Read the Boolean carried by the trusted fold. -/
def globalCheck (a : Acc) : Bool := a.ok

/-- Folding never resurrects a cleared integrity bit: once `ok` is false it stays false. -/
theorem fold_preserves_false (verify : Delta → Bool) (a : Acc) (d : Delta)
    (h : a.ok = false) : (fold verify a d).ok = false := by
  unfold fold
  by_cases hc : verify d = true ∧ d.level > a.state d.frontier
  · rw [if_pos hc]; exact h
  · rw [if_neg hc]

/-- If a single fold leaves the integrity bit set, then the prior bit was set AND this delta verified.
    (The per-step inversion that drives the history-wide soundness theorem.) -/
theorem fold_ok_inv (verify : Delta → Bool) (a : Acc) (d : Delta)
    (h : (fold verify a d).ok = true) : a.ok = true ∧ verify d = true := by
  unfold fold at h
  by_cases hc : verify d = true ∧ d.level > a.state d.frontier
  · rw [if_pos hc] at h; exact ⟨h, hc.1⟩
  · rw [if_neg hc] at h; simp at h

/-- Induction invariant for the trusted fold: if its final Boolean is true,
    every input delta satisfied the supplied Boolean verifier. -/
theorem accumulate_sound (verify : Delta → Bool) :
    ∀ (ds : List Delta) (a : Acc),
      (ds.foldl (fold verify) a).ok = true → a.ok = true ∧ ∀ d ∈ ds, verify d = true := by
  intro ds
  induction ds with
  | nil => intro a h; exact ⟨h, by intro d hd; cases hd⟩
  | cons d ds ih =>
    intro a h
    -- foldl (d :: ds) a = foldl ds (fold verify a d)
    have hstep : (ds.foldl (fold verify) (fold verify a d)).ok = true := h
    obtain ⟨hfold_ok, hrest⟩ := ih (fold verify a d) hstep
    obtain ⟨ha_ok, hd_ok⟩ := fold_ok_inv verify a d hfold_ok
    refine ⟨ha_ok, ?_⟩
    intro e he
    cases he with
    | head => exact hd_ok
    | tail _ he' => exact hrest e he'

/-- Corollary specialized to the initial trusted fold state. -/
theorem globalCheck_sound (verify : Delta → Bool) (ds : List Delta)
    (h : globalCheck (accumulate verify ds) = true) : ∀ d ∈ ds, verify d = true := by
  have h' : (ds.foldl (fold verify) init).ok = true := h
  exact (accumulate_sound verify ds init h').2

/-- The accumulated state never regresses: each fold's resulting state dominates the prior state at
    every frontier (the monotonicity of PoVD, lifted to the accumulator). -/
theorem fold_state_monotone (verify : Delta → Bool) (a : Acc) (d : Delta) :
    ∀ f, a.state f ≤ (fold verify a d).state f := by
  intro f
  unfold fold
  by_cases hc : verify d = true ∧ d.level > a.state d.frontier
  · rw [if_pos hc]
    show a.state f ≤ if f = d.frontier then d.level else a.state f
    split
    · rename_i hf; subst hf; exact Nat.le_of_lt hc.2
    · exact Nat.le_refl _
  · rw [if_neg hc]; exact Nat.le_refl _

/-- Reflexivity of the pure trusted-fold function; not an authority or
    consensus theorem. -/
theorem accumulate_deterministic (verify : Delta → Bool) (ds : List Delta) :
    accumulate verify ds = accumulate verify ds := rfl

end Vela.Accumulation
