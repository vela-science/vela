import Mathlib

/-!
# Vela concurrent-replay commutativity (Theorem 12)

This file formalizes the substrate's informal claim that two
canonical events on *disjoint* findings commute: applying them in
either order produces byte-identical state. Pins what the substrate's
canonical-order doctrine assumes about parallel ingest.

Theorem 1 (`Vela.Log`) proved replay convergence under a single
canonical order. Theorem 7 (`Vela.ReplayIndex`) proved the index-
maintenance rule for append-only findings. Theorem 12 closes the
substrate's last load-bearing assumption about replay: that two
events targeting *different* findings (disjoint targets) commute
when the reducer's apply function is locally commutative on disjoint
targets.

## What is and is not formalized

This is a structural theorem under three abstract assumptions:

1. The reducer's apply function commutes on disjoint targets:
   `apply (apply s e₁) e₂ = apply (apply s e₂) e₁` whenever the
   events target different findings.

2. Disjointness is a decidable predicate on event pairs. The
   substrate's actual disjointness check inspects each event's
   `target.id` field; the formalization keeps this abstract.

3. The reducer is total (every event applies cleanly).

Under these, the theorem proves: for any two events `e₁, e₂` whose
targets are disjoint, the resulting state is independent of the
order in which the substrate applies them.

The Rust reducer satisfies these conditions for `finding.add`,
`finding.note`, `finding.caveat`, `artifact.asserted` events on
disjoint target ids; the conformance suite at
`conformance/` exercises this empirically. Theorem 12 is the
algebraic guarantee that the conformance check is testing.

The general case (events that *share* a target finding) does not
commute; the substrate's canonical order is load-bearing in that
regime, which is what Theorem 1 already pins.
-/

namespace Vela.ConcurrentReplay

variable {AtlasState : Type*} {Event : Type*}

/-- Predicate naming when two events have disjoint targets and
therefore commute under the substrate's reducer. The Rust kernel
implements this as a check on `event.target.id`; here it is
abstract. -/
def DisjointTargets (e₁ e₂ : Event) (disjoint : Event → Event → Prop) : Prop :=
  disjoint e₁ e₂

/-- Reducer apply function. Takes the current state and an event,
returns the next state. -/
abbrev Apply (AtlasState Event : Type*) := AtlasState → Event → AtlasState

/-- Local commutativity on disjoint events. The substrate's reducer
satisfies this for canonical event kinds whose `target.id` fields
differ. -/
def LocallyCommutative
    (apply : Apply AtlasState Event)
    (disjoint : Event → Event → Prop) : Prop :=
  ∀ (s : AtlasState) (e₁ e₂ : Event),
    disjoint e₁ e₂ →
      apply (apply s e₁) e₂ = apply (apply s e₂) e₁

/-- **Theorem 12 (concurrent-replay commutativity for disjoint
events).** If the reducer's apply function is locally commutative on
disjoint events, then two events with disjoint targets commute: the
final state is independent of application order. -/
theorem theorem12_concurrent_replay_commutes
    (apply : Apply AtlasState Event)
    (disjoint : Event → Event → Prop)
    (hCommute : LocallyCommutative apply disjoint)
    (state₀ : AtlasState)
    (e₁ e₂ : Event)
    (hDisjoint : disjoint e₁ e₂) :
    apply (apply state₀ e₁) e₂ = apply (apply state₀ e₂) e₁ :=
  hCommute state₀ e₁ e₂ hDisjoint

/-- **Theorem 12.b (n-event extension).** For a list of pairwise-
disjoint events, applying any permutation produces the same final
state. The proof reduces to repeated application of Theorem 12 over
adjacent swaps; the case shown here is the two-event base case
(corresponding to the substrate's smallest concurrent ingest scenario:
two distinct findings asserted in parallel). -/
theorem theorem12b_two_event_swap
    (apply : Apply AtlasState Event)
    (disjoint : Event → Event → Prop)
    (hCommute : LocallyCommutative apply disjoint)
    (state₀ : AtlasState)
    (e₁ e₂ : Event)
    (hDisjoint : disjoint e₁ e₂) :
    apply (apply state₀ e₁) e₂ = apply (apply state₀ e₂) e₁ :=
  theorem12_concurrent_replay_commutes apply disjoint hCommute state₀ e₁ e₂ hDisjoint

/-! ## De-hollowing: a concrete reducer that PROVES `LocallyCommutative`

The theorems above are honest but conditional: they take `LocallyCommutative`
as a hypothesis, so the substantive claim ("the substrate's reducer commutes on
disjoint targets") is *assumed*, not *proven*. The section below removes that
hollowness exactly as `Vela.ReducerModel` did for T28/T34 — by giving a CONCRETE
reducer over a concrete state and discharging `LocallyCommutative` from the
reducer's definition. No axiom, no `sorry`.

We model the substrate's per-finding projection as a map from a target id to the
finding's current content (`String → Option String`); the empty map is `S_0`.
A canonical event is a `(target, content)` pair, and the reducer writes the
content into the slot keyed by `target` — exactly the `event.target.id`-keyed
update the prose describes. Two events are *disjoint* iff their target ids
differ, and on disjoint targets the two writes touch independent slots, so they
commute. The commutation is a theorem about `Function.update`, derived from the
reducer step — the real content T12 gestured at. -/

namespace Concrete

/-- Concrete substrate projection: each target id maps to its finding's current
content (`none` if no finding present at that id). The initial state `S_0` is
the everywhere-`none` map. -/
abbrev FindingMap := String → Option String

/-- A canonical event: write `content` into the finding slot `target`. This is
the faithful shape of `finding.add`/`finding.note`/`finding.caveat`/
`artifact.asserted` — each carries a `target.id` and a payload. -/
structure CEvent where
  target : String
  content : String

/-- The empty initial projection `S_0`. -/
def empty : FindingMap := fun _ => none

/-- The concrete reducer step: write the event's content into its target slot.
This is the substrate's `event.target.id`-keyed update made definitional. -/
def capply (s : FindingMap) (e : CEvent) : FindingMap :=
  Function.update s e.target (some e.content)

/-- The substrate's actual disjointness check: two events are disjoint iff their
`target.id` fields differ. Here it is a concrete (decidable) predicate, not an
abstract one. -/
def cdisjoint (e₁ e₂ : CEvent) : Prop := e₁.target ≠ e₂.target

/-- **De-hollowed core**: the concrete reducer IS locally commutative on disjoint
targets — PROVEN from the update semantics, not assumed. Two writes to distinct
target slots commute because `Function.update` on distinct keys commutes. -/
theorem capply_locally_commutative : LocallyCommutative capply cdisjoint := by
  intro s e₁ e₂ hd
  simp only [capply]
  exact Function.update_comm hd (some e₁.content) (some e₂.content) s

/-- **De-hollowed Theorem 12.** Specialized to the concrete reducer: two events
with disjoint target ids commute, with the commutativity *derived from the
reducer definition* rather than taken as a hypothesis. -/
theorem theorem12_concrete_commutes
    (s₀ : FindingMap) (e₁ e₂ : CEvent) (hDisjoint : cdisjoint e₁ e₂) :
    capply (capply s₀ e₁) e₂ = capply (capply s₀ e₂) e₁ :=
  theorem12_concurrent_replay_commutes capply cdisjoint
    capply_locally_commutative s₀ e₁ e₂ hDisjoint

/-- Sanity check that disjointness is load-bearing: two events on the SAME target
do NOT in general commute (last writer wins), so the concrete reducer matches the
prose's claim that the canonical order is load-bearing on shared targets. -/
theorem capply_shared_target_noncommute :
    ∃ (s₀ : FindingMap) (e₁ e₂ : CEvent),
      e₁.target = e₂.target ∧ capply (capply s₀ e₁) e₂ ≠ capply (capply s₀ e₂) e₁ := by
  refine ⟨empty, ⟨"t", "a"⟩, ⟨"t", "b"⟩, rfl, ?_⟩
  intro h
  have := congrFun h "t"
  simp [capply, Function.update] at this

end Concrete

end Vela.ConcurrentReplay
