import Vela.Accumulation.HeteroAccumulation

/-!
# Historical composition of trusted-fold invariants

This compatibility module combines two toy-model invariants: a Boolean carried
by a trusted fold and a `Verified` predicate whose transfer constructor assumes
the relevant preservation property. Its theorems are valid for those
definitions. They do not supply a cryptographic proof, succinct verification,
an externally checkable history commitment, transfer soundness for an actual
bridge, or a Vela protocol guarantee. The historical names remain for exact
research reproduction; this module is not imported by the active theorem
aggregate.
-/

namespace Vela.ProtocolKeystone

open Vela.HeteroAccumulation

/-- The history was fully accepted from accumulator `a`: folding the deltas never hit a rejection. -/
def AllAccepted (nv : Frontier → Level → Bool) (lk : Frontier → Frontier → Option (Level → Level)) :
    Acc → List Delta → Prop
  | _, [] => True
  | a, d :: ds => (accept nv lk a.state d).isSome ∧ AllAccepted nv lk (fold nv lk a d) ds

/-- One fold step that leaves the integrity bit set must have ACCEPTED its delta (rejection clears the
    bit), and the prior bit was already set. -/
theorem fold_ok_step (nv : Frontier → Level → Bool) (lk : Frontier → Frontier → Option (Level → Level))
    (a : Acc) (d : Delta) (h : (fold nv lk a d).ok = true) :
    a.ok = true ∧ (accept nv lk a.state d).isSome := by
  cases hacc : accept nv lk a.state d with
  | none => simp [fold, hacc] at h
  | some s =>
    simp only [fold, hacc] at h
    exact ⟨h, by simp⟩

/-- Induction invariant for the historical trusted fold. This is not a
    cryptographic history certificate. -/
theorem ok_implies_all_accepted
    (nv : Frontier → Level → Bool) (lk : Frontier → Frontier → Option (Level → Level)) :
    ∀ (ds : List Delta) (a : Acc),
      (ds.foldl (fold nv lk) a).ok = true → a.ok = true ∧ AllAccepted nv lk a ds := by
  intro ds
  induction ds with
  | nil => intro a h; exact ⟨h, trivial⟩
  | cons d ds ih =>
    intro a h
    obtain ⟨hfold_ok, hrest⟩ := ih (fold nv lk a d) h
    obtain ⟨ha_ok, hacc⟩ := fold_ok_step nv lk a d hfold_ok
    exact ⟨ha_ok, hacc, hrest⟩

/-- Compose the trusted-fold invariant with the model's assumed `Verified`
    predicate. The result is scoped to these definitions and is not a Vela
    protocol, transfer-soundness, or succinct-verification theorem. -/
theorem protocol_keystone
    (nv : Frontier → Level → Bool) (lk : Frontier → Frontier → Option (Level → Level))
    (ds : List Delta) (h : (accumulate nv lk ds).ok = true) :
    AllAccepted nv lk init ds ∧ StateVerified nv lk (accumulate nv lk ds).state := by
  refine ⟨?_, accumulate_state_verified nv lk ds⟩
  exact (ok_implies_all_accepted nv lk ds init h).2

/-- Reflexivity of the pure historical model; not an authority or consensus
    theorem. -/
theorem keystone_deterministic
    (nv : Frontier → Level → Bool) (lk : Frontier → Frontier → Option (Level → Level))
    (ds : List Delta) : accumulate nv lk ds = accumulate nv lk ds := rfl

end Vela.ProtocolKeystone
