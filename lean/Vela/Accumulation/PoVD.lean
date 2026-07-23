/-!
# Historical monotone-improvement policy model (formerly PoVD)

This compatibility module models a local deterministic rule over one supplied
Boolean verifier and one numeric frontier level:

* a contribution is a `Delta` proposing to raise a frontier to a new `level`, backed by a `witness`;
* it is accepted iff the verifier passes and the level strictly increases; and
* a stale or duplicate delta is rejected by that local rule.

The theorems establish only those properties for this concrete model. They do
not establish network consensus, agreement across different event sets,
personhood, Sybil resistance, fraud resistance, fair credit, or authority-free
scientific acceptance. The historical name and declarations remain for exact
research reproduction; this module is not imported by the active theorem
aggregate.
-/

namespace Vela.PoVD

/-- Numeric identifiers and levels in the historical toy model. -/
abbrev Frontier := Nat
abbrev Level := Nat

/-- The model state: one numeric level per identifier. -/
abbrev State := Frontier → Level

/-- The all-zero initial state. -/
def empty : State := fun _ => 0

/-- A contribution: raise `frontier` to `level`, backed by `witness`. -/
structure Delta where
  frontier : Frontier
  level    : Level
  witness  : Nat

/-- Local transition rule parameterized by an arbitrary Boolean `verify`. The
    rule returns a new state iff the Boolean is true and the numeric level
    strictly increases. -/
def accept (verify : Delta → Bool) (S : State) (d : Delta) : Option State :=
  if verify d = true ∧ d.level > S d.frontier then
    some (fun f => if f = d.frontier then d.level else S f)
  else none

/-- Historical compatibility name for whether the local rule returns a state. -/
def credited (verify : Delta → Bool) (S : State) (d : Delta) : Bool :=
  (accept verify S d).isSome

/-- If the local rule returns a state, the supplied Boolean was true. -/
theorem accept_implies_verified
    (verify : Delta → Bool) (S : State) (d : Delta) (S' : State)
    (h : accept verify S d = some S') : verify d = true := by
  unfold accept at h
  by_cases hc : verify d = true ∧ d.level > S d.frontier
  · exact hc.1
  · rw [if_neg hc] at h; simp at h

/-- A successful local transition never lowers a numeric level. -/
theorem accept_monotone
    (verify : Delta → Bool) (S : State) (d : Delta) (S' : State)
    (h : accept verify S d = some S') : ∀ f, S f ≤ S' f := by
  unfold accept at h
  by_cases hc : verify d = true ∧ d.level > S d.frontier
  · rw [if_pos hc] at h
    injection h with h; subst h
    obtain ⟨_, hlt⟩ := hc
    intro f
    show S f ≤ if f = d.frontier then d.level else S f
    split
    · rename_i hf; subst hf; exact Nat.le_of_lt hlt
    · exact Nat.le_refl _
  · rw [if_neg hc] at h; simp at h

/-- A delta whose level does not strictly increase is rejected by this local
    rule. This is numeric stale-value rejection, not a double-spend theorem. -/
theorem stale_rejected
    (verify : Delta → Bool) (S : State) (d : Delta)
    (h : d.level ≤ S d.frontier) : accept verify S d = none := by
  unfold accept
  rw [if_neg (by rintro ⟨_, hlt⟩; exact absurd hlt (Nat.not_lt.mpr h))]

/-- Repeating the same delta after this local rule accepted it is stale and is
    therefore rejected. This is duplicate suppression, not Sybil resistance. -/
theorem duplicate_rejected
    (verify : Delta → Bool) (S : State) (d : Delta) (S' : State)
    (h : accept verify S d = some S') : accept verify S' d = none := by
  have hlevel : S' d.frontier = d.level := by
    unfold accept at h
    by_cases hc : verify d = true ∧ d.level > S d.frontier
    · rw [if_pos hc] at h; injection h with h; subst h; simp
    · rw [if_neg hc] at h; simp at h
  exact stale_rejected verify S' d (by simp [hlevel])

/-- Reflexivity of the local pure acceptance function. This is not a consensus
    or authority theorem. -/
theorem accept_deterministic
    (verify : Delta → Bool) (S : State) (d : Delta) :
    accept verify S d = accept verify S d := rfl

/-- The historical `credited` Boolean is true only when the supplied Boolean
    is true and the proposed level strictly increases. -/
theorem credited_is_real
    (verify : Delta → Bool) (S : State) (d : Delta)
    (h : credited verify S d = true) :
    verify d = true ∧ d.level > S d.frontier := by
  unfold credited accept at h
  by_cases hc : verify d = true ∧ d.level > S d.frontier
  · exact hc
  · rw [if_neg hc] at h; simp at h

end Vela.PoVD
