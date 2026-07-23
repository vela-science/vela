/-!
# Predicate-preserving maps between abstract frontiers

This historical compatibility module packages a type with a proposition and a
map with an explicit proof field saying that the proposition is preserved.
`transfer_sound` projects that supplied field; it does not discover or verify a
bridge, connect repositories, transport Vela standing, or refine an executable
verifier. Identity and composition establish the ordinary laws of these
property-carrying functions at their object maps.

Concrete declarations later in this file prove small combinatorial lemmas at
their exact theorem surfaces. Each must be judged from its own definitions and
hypotheses. The retained historical theorem names do not create a general
cross-frontier protocol guarantee or scientific-transfer authority.
-/

namespace Vela

universe u v w

/-- A frontier: a type of candidate objects together with a frozen verification predicate. -/
structure Frontier where
  Obj : Type u
  verified : Obj → Prop

/-- A property-preserving map. Preservation is data supplied in `sound`; the
    structure does not prove or validate that premise externally. -/
structure Transfer (A B : Frontier) where
  toFun : A.Obj → B.Obj
  sound : ∀ o : A.Obj, A.verified o → B.verified (toFun o)

/-- Project the predicate-preservation proof carried by `T`. -/
theorem transfer_sound {A B : Frontier} (T : Transfer A B)
    {o : A.Obj} (h : A.verified o) : B.verified (T.toFun o) :=
  T.sound o h

/-- The identity transfer (every object to itself; verification preserved trivially). -/
def Transfer.id (A : Frontier) : Transfer A A where
  toFun := fun o => o
  sound := fun _ h => h

/-- Property-preserving maps compose. -/
def Transfer.comp {A B C : Frontier} (S : Transfer A B) (T : Transfer B C) : Transfer A C where
  toFun := fun o => T.toFun (S.toFun o)
  sound := fun o h => T.sound _ (S.sound o h)

/-- Composition of transfers is function composition on objects (functoriality on objects). -/
@[simp] theorem Transfer.comp_toFun {A B C : Frontier}
    (S : Transfer A B) (T : Transfer B C) (o : A.Obj) :
    (S.comp T).toFun o = T.toFun (S.toFun o) := rfl

/-- Identity is a left unit for composition (on objects). -/
@[simp] theorem Transfer.id_comp {A B : Frontier} (T : Transfer A B) (o : A.Obj) :
    ((Transfer.id A).comp T).toFun o = T.toFun o := rfl

/-- Identity is a right unit for composition (on objects). -/
@[simp] theorem Transfer.comp_id {A B : Frontier} (T : Transfer A B) (o : A.Obj) :
    (T.comp (Transfer.id B)).toFun o = T.toFun o := rfl

/-- Composition is associative (on objects). -/
@[simp] theorem Transfer.comp_assoc {A B C D : Frontier}
    (R : Transfer A B) (S : Transfer B C) (T : Transfer C D) (o : A.Obj) :
    ((R.comp S).comp T).toFun o = (R.comp (S.comp T)).toFun o := rfl

/-- If the supplied preservation proof and an additional predicate `q` both
    hold for the mapped object, that object witnesses their conjunction in
    `B`. This is a local logical consequence, not a Vela correction or standing
    transition. -/
theorem transfer_closes {A B : Frontier} (T : Transfer A B)
    (q : B.Obj → Prop) {o : A.Obj}
    (h : A.verified o) (hq : q (T.toFun o)) :
    ∃ b : B.Obj, B.verified b ∧ q b :=
  ⟨T.toFun o, transfer_sound T h, hq⟩

/-- Sanity check for the identity property-preserving map. -/
example (A : Frontier) {o : A.Obj} (h : A.verified o) :
    A.verified ((Transfer.id A).toFun o) := transfer_sound (Transfer.id A) h

/-! ## A concrete property-preservation lemma

The generic theorem above merely projects a field. The following declaration
instead proves, for the exact `SidonList` predicate below, that translation
preserves that predicate. It says nothing about Vela records, accepted state,
or an executable verifier unless a separate refinement theorem supplies that
connection. -/

/-- A Sidon set over `List Nat`: every coincidence of pairwise sums comes from the same pair. -/
def SidonList (S : List Nat) : Prop :=
  ∀ a ∈ S, ∀ b ∈ S, ∀ c ∈ S, ∀ d ∈ S, a + b = c + d → (a = c ∧ b = d) ∨ (a = d ∧ b = c)

/-- The integer-Sidon frontier. -/
def sidonFrontier : Frontier := { Obj := List Nat, verified := SidonList }

/-- Translation preserves the Sidon property -- a genuine, computed soundness lemma. -/
theorem sidon_translate_sound (t : Nat) {S : List Nat} (h : SidonList S) :
    SidonList (S.map (· + t)) := by
  intro a ha b hb c hc d hd hsum
  rw [List.mem_map] at ha hb hc hd
  obtain ⟨a', ha', rfl⟩ := ha
  obtain ⟨b', hb', rfl⟩ := hb
  obtain ⟨c', hc', rfl⟩ := hc
  obtain ⟨d', hd', rfl⟩ := hd
  have hsum' : a' + b' = c' + d' := by omega
  rcases h a' ha' b' hb' c' hc' d' hd' hsum' with ⟨h1, h2⟩ | ⟨h1, h2⟩
  · exact Or.inl ⟨by omega, by omega⟩
  · exact Or.inr ⟨by omega, by omega⟩

/-- Package the exact translation lemma as a property-preserving map. -/
def translateTransfer (t : Nat) : Transfer sidonFrontier sidonFrontier where
  toFun := fun S => S.map (· + t)
  sound := fun _ h => sidon_translate_sound t h

/-- Apply the exact translation lemma through the retained generic interface. -/
example {S : List Nat} (h : SidonList S) (t : Nat) :
    sidonFrontier.verified ((translateTransfer t).toFun S) :=
  transfer_sound (translateTransfer t) h

/-! ## An equivalence between two predicates (distinct sums ⇄ distinct differences)

The translation example stays within one predicate. Here two predicates on the
same `List Nat` carrier are related by exact reindexing lemmas. A **Golomb
ruler** is a set whose pairwise differences are distinct; written additively
(to avoid truncated subtraction), `a - b = c - d` becomes
`a + d = b + c`. The proofs establish both implications for these definitions.
They do not by themselves equate repository records, verifier identities, or
scientific standing.
-/

/-- A Golomb-ruler set over `List Nat`: every coincidence of pairwise differences (in additive form)
    comes from the same pair. -/
def GolombList (S : List Nat) : Prop :=
  ∀ a ∈ S, ∀ b ∈ S, ∀ c ∈ S, ∀ d ∈ S, a + d = b + c → (a = b ∧ c = d) ∨ (a = c ∧ b = d)

/-- The distinct-differences (Golomb-ruler) frontier. -/
def golombFrontier : Frontier := { Obj := List Nat, verified := GolombList }

/-- **Distinct sums ⇒ distinct differences.** A real reindexing of the Sidon hypothesis: instantiate
    `a + b = c + d` at the permuted arguments `(a, d, b, c)`. No `Nat` subtraction, no axiom. -/
theorem sidon_to_golomb_sound {S : List Nat} (h : SidonList S) : GolombList S := by
  intro a ha b hb c hc d hd hdiff
  -- hdiff : a + d = b + c, which is the Sidon premise at (a, d, b, c)
  rcases h a ha d hd b hb c hc hdiff with ⟨h1, h2⟩ | ⟨h1, h2⟩
  · exact Or.inl ⟨h1, by omega⟩
  · exact Or.inr ⟨by omega, by omega⟩

/-- **Distinct differences ⇒ distinct sums.** The converse reindexing, so the bridge is an isomorphism
    of verifiers, not just a one-way map. -/
theorem golomb_to_sidon_sound {S : List Nat} (h : GolombList S) : SidonList S := by
  intro a ha b hb c hc d hd hsum
  -- hsum : a + b = c + d, which is the Golomb premise at (a, d, c, b)
  have hg : a + b = c + d := hsum
  rcases h a ha d hd c hc b hb (by omega) with ⟨h1, h2⟩ | ⟨h1, h2⟩
  · exact Or.inr ⟨by omega, by omega⟩
  · exact Or.inl ⟨h1, by omega⟩

/-- Package `sidon_to_golomb_sound` as an identity-on-values
    property-preserving map. -/
def sidonToGolomb : Transfer sidonFrontier golombFrontier where
  toFun := fun S => S
  sound := fun _ h => sidon_to_golomb_sound h

/-- The reverse transfer `Golomb → Sidon`. -/
def golombToSidon : Transfer golombFrontier sidonFrontier where
  toFun := fun S => S
  sound := fun _ h => golomb_to_sidon_sound h

/-- Apply the exact predicate implication to the same underlying list. -/
example {S : List Nat} (h : SidonList S) : golombFrontier.verified (sidonToGolomb.toFun S) :=
  transfer_sound sidonToGolomb h

/-- The two packaged maps are both identity on the underlying list. -/
@[simp] theorem sidon_golomb_roundtrip {S : List Nat} :
    (golombToSidon.toFun (sidonToGolomb.toFun S)) = S := rfl

end Vela
