import Mathlib

/-!
The Formal Conjectures declarations `OeisA303656.conjecture` needs, copied so
that the statement requires Mathlib and nothing else. Dependencies
come before the declarations that use them:

* `OeisA303656.A`
-/

namespace OeisA303656
end OeisA303656

-- OeisA303656.A, from FormalConjectures/OEIS/303656.lean
noncomputable section
namespace OeisA303656

/-- The predicate that `n` can be written as $a^2 + b^2 + 3^c + 5^d$ for nonnegative integers. -/
def A (n : ℕ) : Prop :=
  ∃ a b c d : ℕ, n = a ^ 2 + b ^ 2 + 3 ^ c + 5 ^ d

end OeisA303656
end

open OeisA303656
