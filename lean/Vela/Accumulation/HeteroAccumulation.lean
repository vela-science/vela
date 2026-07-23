/-!
# Historical heterogeneous trusted-fold model

This compatibility module studies a fold whose deltas may be justified two
ways:

* **natively**, by a witness the frontier's own frozen verifier accepts; or
* **by transfer**, applying a function supplied by a lookup and carrying the
  result through the `Verified.transfer` constructor.

`Verified.transfer` encodes preservation in its constructor. The resulting
theorems show that the fold retains this assumed predicate; they do not prove a
real transfer certificate sound, prevent laundering outside the toy model, or
establish Vela authority. The source remains for historical reproduction and
is not imported by the active theorem aggregate.

## Honest scope
* `Verified.transfer` encodes its soundness premise as a constructor; this file
  does not prove any individual transfer or registry sound.
* A transfer imports the source's *current best* (`S src`); downward-closure of
  weaker levels, cryptographic proof carrying, and adoption are outside this model.
-/

namespace Vela.HeteroAccumulation

abbrev Frontier := Nat
abbrev Level := Nat
abbrev State := Frontier → Level

/-- How a delta's claim is justified. -/
inductive Justification where
  | native (witness : Nat)   -- backed by a native witness for this frontier's verifier
  | transfer (src : Frontier) -- imported from `src`'s current verified best via a registered transfer

/-- A contribution: raise `frontier` to `level`, justified `just`. -/
structure Delta where
  frontier : Frontier
  level    : Level
  just     : Justification

/-- The model's inductive `Verified` predicate. `nv f L` is the supplied
    Boolean premise at frontier `f`; `lk src dst` returns an optional function.
    The transfer constructor assumes the predicate follows that function. It
    does not prove that a concrete external transfer is valid. -/
inductive Verified (nv : Frontier → Level → Bool) (lk : Frontier → Frontier → Option (Level → Level)) :
    Frontier → Level → Prop where
  | native  {f L} : nv f L = true → Verified nv lk f L
  | transfer {src dst : Frontier} {g : Level → Level} {L : Level} :
      Verified nv lk src L → lk src dst = some g → Verified nv lk dst (g L)

/-- Raise the state at one frontier, leaving the rest unchanged. -/
def raise (S : State) (f : Frontier) (L : Level) : State :=
  fun f' => if f' = f then L else S f'

/-- Acceptance in the toy model, parameterized by `nv` and the lookup `lk`.
    A native delta is accepted iff its witness verifies and it strictly improves; a transfer delta is
    accepted iff a registered transfer maps the source's *current verified best* to exactly the claimed
    level, the source is itself nonzero (already verified), and it strictly improves. -/
def accept (nv : Frontier → Level → Bool) (lk : Frontier → Frontier → Option (Level → Level))
    (S : State) (d : Delta) : Option State :=
  match d.just with
  | .native _ =>
      if nv d.frontier d.level = true ∧ d.level > S d.frontier then
        some (raise S d.frontier d.level)
      else none
  | .transfer src =>
      match lk src d.frontier with
      | some g =>
          if 0 < S src ∧ g (S src) = d.level ∧ d.level > S d.frontier then
            some (raise S d.frontier d.level)
          else none
      | none => none

/-- The trusted fold state; no storage-size or proof-size bound is established. -/
structure Acc where
  state : State
  ok    : Bool

def init : Acc := { state := fun _ => 0, ok := true }

def fold (nv : Frontier → Level → Bool) (lk : Frontier → Frontier → Option (Level → Level))
    (a : Acc) (d : Delta) : Acc :=
  match accept nv lk a.state d with
  | some s => { state := s, ok := a.ok }
  | none   => { state := a.state, ok := false }

def accumulate (nv : Frontier → Level → Bool) (lk : Frontier → Frontier → Option (Level → Level))
    (ds : List Delta) : Acc :=
  ds.foldl (fold nv lk) init

/-- The state invariant: every nonzero entry inhabits the model's declared
    `Verified` predicate. -/
def StateVerified (nv : Frontier → Level → Bool) (lk : Frontier → Frontier → Option (Level → Level))
    (S : State) : Prop :=
  ∀ f, 0 < S f → Verified nv lk f (S f)

/-- One accepted delta preserves the model's inductive predicate, including
    the branch whose preservation premise is carried by `Verified.transfer`. -/
theorem accept_preserves_verified
    (nv : Frontier → Level → Bool) (lk : Frontier → Frontier → Option (Level → Level))
    (S S' : State) (d : Delta)
    (hinv : StateVerified nv lk S) (h : accept nv lk S d = some S') :
    StateVerified nv lk S' := by
  intro f hf
  unfold accept at h
  cases hj : d.just with
  | native w =>
    rw [hj] at h; dsimp only at h
    by_cases hc : nv d.frontier d.level = true ∧ d.level > S d.frontier
    · rw [if_pos hc] at h
      injection h with h; subst h
      show Verified nv lk f (raise S d.frontier d.level f)
      by_cases hfe : f = d.frontier
      · subst hfe
        simp only [raise]
        exact Verified.native hc.1
      · simp only [raise, if_neg hfe]
        simp only [raise, if_neg hfe] at hf
        exact hinv f hf
    · rw [if_neg hc] at h; simp at h
  | transfer src =>
    rw [hj] at h; dsimp only at h
    cases hlk : lk src d.frontier with
    | none => rw [hlk] at h; simp at h
    | some g =>
      rw [hlk] at h; dsimp only at h
      by_cases hc : 0 < S src ∧ g (S src) = d.level ∧ d.level > S d.frontier
      · rw [if_pos hc] at h
        injection h with h; subst h
        obtain ⟨hsrc, hmap, _⟩ := hc
        show Verified nv lk f (raise S d.frontier d.level f)
        by_cases hfe : f = d.frontier
        · subst hfe
          simp only [raise]
          -- Apply the preservation constructor assumed by the toy model.
          have hv : Verified nv lk src (S src) := hinv src hsrc
          have hvt : Verified nv lk d.frontier (g (S src)) := Verified.transfer hv hlk
          rw [hmap] at hvt; exact hvt
        · simp only [raise, if_neg hfe]
          simp only [raise, if_neg hfe] at hf
          exact hinv f hf
      · rw [if_neg hc] at h; simp at h

/-- One fold step preserves the state invariant (accept-or-reject: rejection leaves state untouched). -/
theorem fold_preserves_verified
    (nv : Frontier → Level → Bool) (lk : Frontier → Frontier → Option (Level → Level))
    (a : Acc) (d : Delta) (hinv : StateVerified nv lk a.state) :
    StateVerified nv lk (fold nv lk a d).state := by
  unfold fold
  cases h : accept nv lk a.state d with
  | some s =>
    show StateVerified nv lk s
    exact accept_preserves_verified nv lk a.state s d hinv h
  | none =>
    show StateVerified nv lk a.state
    exact hinv

/-- Induction over the trusted fold: every nonzero entry in the result inhabits
    the model's `Verified` predicate. The transfer case is conditional on the
    predicate-preservation constructor built into that definition. -/
theorem accumulate_state_verified
    (nv : Frontier → Level → Bool) (lk : Frontier → Frontier → Option (Level → Level))
    (ds : List Delta) :
    StateVerified nv lk (accumulate nv lk ds).state := by
  have gen : ∀ (ds : List Delta) (a : Acc),
      StateVerified nv lk a.state → StateVerified nv lk (ds.foldl (fold nv lk) a).state := by
    intro ds
    induction ds with
    | nil => intro a hinv; exact hinv
    | cons d ds ih =>
      intro a hinv
      exact ih (fold nv lk a d) (fold_preserves_verified nv lk a d hinv)
  have hbase : StateVerified nv lk init.state := by
    intro f hf; simp only [init] at hf; exact absurd hf (Nat.lt_irrefl 0)
  exact gen ds init hbase

/-- Reflexivity of the pure toy-model fold; not an authority or consensus
    theorem. -/
theorem accumulate_deterministic
    (nv : Frontier → Level → Bool) (lk : Frontier → Frontier → Option (Level → Level))
    (ds : List Delta) : accumulate nv lk ds = accumulate nv lk ds := rfl

/-! ## A concrete constructor example

The example constructs the model's `Verified` predicate at frontier 1 from a
native premise at frontier 0 and an identity function returned by the lookup.
It demonstrates the inductive constructor only; it is not evidence for a real
scientific transfer or discovery. -/

private def nvDemo : Frontier → Level → Bool := fun f L => (f == 0) && (L == 5)
private def lkDemo : Frontier → Frontier → Option (Level → Level) :=
  fun s d => if (s == 0) && (d == 1) then some id else none

example : Verified nvDemo lkDemo 1 5 :=
  Verified.transfer (src := 0) (dst := 1) (g := id) (L := 5)
    (Verified.native (rfl : nvDemo 0 5 = true)) (rfl : lkDemo 0 1 = some id)

end Vela.HeteroAccumulation
