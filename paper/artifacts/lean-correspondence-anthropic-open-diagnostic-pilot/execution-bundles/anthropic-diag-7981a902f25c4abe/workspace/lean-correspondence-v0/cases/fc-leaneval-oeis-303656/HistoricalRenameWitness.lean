import Mathlib

/-!
The source change at Formal Conjectures commit
81e700d16ada8c5e63339d7a16accb672d3067ae renamed the helper definition while
leaving its body unchanged. This witness covers that exact predicate body. It
does not generalize to arbitrary future source edits.
-/

namespace LeanCorrespondence.OeisA303656

def IsSumOfTwoSquaresAndPowersOf3And5 (n : ℕ) : Prop :=
  ∃ a b c d : ℕ, n = a ^ 2 + b ^ 2 + 3 ^ c + 5 ^ d

def A (n : ℕ) : Prop :=
  ∃ a b c d : ℕ, n = a ^ 2 + b ^ 2 + 3 ^ c + 5 ^ d

theorem helper_rename_preserves_predicate (n : ℕ) :
    IsSumOfTwoSquaresAndPowersOf3And5 n ↔ A n := Iff.rfl

theorem conjecture_type_preserved :
    (∀ n : ℕ, 1 < n → IsSumOfTwoSquaresAndPowersOf3And5 n) ↔
      (∀ n : ℕ, 1 < n → A n) := Iff.rfl

end LeanCorrespondence.OeisA303656
