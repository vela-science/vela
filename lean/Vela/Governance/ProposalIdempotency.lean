import Mathlib

/-!
# Vela proposal-acceptance idempotency (Theorem 14)

This file formalizes the substrate's claim that accepting the
same `vpr_*` proposal twice is a no-op on frontier state.

The substrate's reducer deduplicates accepted proposals by their
`applied_event_id`: once a proposal's accept-event id has been
recorded in the frontier's accepted-set, re-applying the same
acceptance produces no further state change. Theorem 14 names
this guarantee algebraically.

## What is and is not formalized

This is a structural theorem under one abstract predicate:

  `DedupedOn (accept : S → P → S) (s : S) (p : P) : Prop`

— the substrate's "this proposal has already been accepted in
this state" predicate. We model the reducer as a function
`accept : FrontierState → Proposal → FrontierState` and the
substrate's deduplication policy as the hypothesis:

  `∀ s p. DedupedOn accept s p → accept s p = s`

Under this, the iterated application `accept (accept s p) p`
equals `accept s p` whenever `accept s p` falls under the
deduplication predicate. The proof is one line.

## Substrate role

The Rust reducer at `crates/vela-protocol/src/reducer.rs`
implements proposal acceptance with an applied-event-id
deduplication step: a `proposal.accepted` event whose
`applied_event_id` already appears in the frontier's accepted-
set is a no-op (the event is recorded for replay determinism
but produces no further projection-state changes). Theorem 14
makes that dedup assumption explicit and proves that under it
the reducer is idempotent on repeat acceptances.

This pins the substrate's replay-safety claim: re-applying the
same accepted-proposal event during a replay (or a federation
re-sync) cannot diverge from the canonical state.
-/

namespace Vela.ProposalIdempotency

variable {S : Type*} {P : Type*}

/-- The substrate's "this proposal has already been accepted
in this state" predicate. Concretely realized in the reducer
as `s.accepted_proposals.contains(p.applied_event_id)`. -/
def DedupedOn (accept : S → P → S) (s : S) (p : P) : Prop :=
  accept s p = s

/-- **Theorem 14 (proposal-acceptance idempotency).** Under the
substrate's deduplication policy — namely that once a proposal
has been accepted in a state, re-applying its acceptance is a
no-op on that state — the reducer is idempotent on repeated
acceptance of the same proposal: `accept (accept s p) p =
accept s p`.

The proof is one line: the deduplication hypothesis says
`accept (accept s p) p = accept s p` whenever
`DedupedOn accept (accept s p) p` holds, which is the post-
acceptance state's defining property under the substrate's
dedup rule (accepting `p` from a state where `p` is already
accepted is a no-op). -/
theorem theorem14_accept_idempotent
    (accept : S → P → S) (s : S) (p : P)
    (hDedup : DedupedOn accept (accept s p) p) :
    accept (accept s p) p = accept s p := by
  exact hDedup

/-- **Threefold corollary.** A direct unfolding of Theorem 14:
applying `accept` three times in a row is the same as applying
it once. Substrate role: a replay that re-emits the acceptance
event multiple times in succession (e.g. during a federation
re-sync that bursts the same event) cannot diverge from a
single acceptance after the first repeat. -/
theorem theorem14_accept_threefold
    (accept : S → P → S) (s : S) (p : P)
    (hDedup : DedupedOn accept (accept s p) p) :
    accept (accept (accept s p) p) p = accept s p := by
  rw [theorem14_accept_idempotent accept s p hDedup]
  exact hDedup

/-! ## De-hollowing: a concrete reducer that PROVES the dedup hypothesis

The theorems above are honest but conditional: `DedupedOn accept (accept s p) p`
(literally `accept (accept s p) p = accept s p`) is taken as a hypothesis, so the
substantive idempotency claim is *assumed*, not *proven*. The section below
removes that hollowness exactly as `Vela.ReducerModel` did for T28/T34 — by
giving a CONCRETE acceptance reducer over a concrete frontier state and PROVING
the dedup property (and therefore idempotency) from the reducer's definition.
No axiom, no `sorry`.

We model the frontier as its accepted-set: the list of `applied_event_id`s that
have been accepted. Accepting a proposal whose id is already present is a no-op
(the substrate's `applied_event_id` dedup step, `crates/vela-protocol/src/
reducer.rs`); otherwise the id is appended. The idempotency then *follows* from
the definition, because after the first accept the id is in the set. -/

namespace Concrete

/-- Concrete frontier projection: the set of accepted `applied_event_id`s,
in acceptance order (append-only). `S_0` is the empty list. -/
abbrev Accepted := List String

/-- A proposal, identified by its `applied_event_id` (the dedup key). -/
structure Proposal where
  appliedEventId : String

/-- The concrete acceptance reducer: appending a proposal's id to the
accepted-set, but *only if it is not already present* — the substrate's
`applied_event_id` deduplication step made definitional. -/
def caccept (s : Accepted) (p : Proposal) : Accepted :=
  if p.appliedEventId ∈ s then s else s ++ [p.appliedEventId]

/-- After accepting `p`, its id is in the accepted-set — whether it was already
present (guard true, set unchanged) or freshly appended (guard false). -/
theorem mem_after_accept (s : Accepted) (p : Proposal) :
    p.appliedEventId ∈ caccept s p := by
  unfold caccept
  split
  · assumption
  · simp

/-- **De-hollowed dedup property**: re-accepting an already-accepted proposal is a
no-op — PROVEN from the reducer definition (the guard fires once the id is in the
set), not assumed. This is exactly `DedupedOn caccept (caccept s p) p`. -/
theorem caccept_deduped (s : Accepted) (p : Proposal) :
    DedupedOn caccept (caccept s p) p := by
  show caccept (caccept s p) p = caccept s p
  rw [caccept, if_pos (mem_after_accept s p)]

/-- **De-hollowed Theorem 14.** Idempotency of the concrete acceptance reducer,
with the dedup hypothesis *discharged* by `caccept_deduped` rather than assumed:
`caccept (caccept s p) p = caccept s p` holds unconditionally. -/
theorem theorem14_concrete_accept_idempotent (s : Accepted) (p : Proposal) :
    caccept (caccept s p) p = caccept s p :=
  theorem14_accept_idempotent caccept s p (caccept_deduped s p)

/-- **De-hollowed threefold corollary**, likewise unconditional. -/
theorem theorem14_concrete_accept_threefold (s : Accepted) (p : Proposal) :
    caccept (caccept (caccept s p) p) p = caccept s p :=
  theorem14_accept_threefold caccept s p (caccept_deduped s p)

end Concrete

end Vela.ProposalIdempotency
