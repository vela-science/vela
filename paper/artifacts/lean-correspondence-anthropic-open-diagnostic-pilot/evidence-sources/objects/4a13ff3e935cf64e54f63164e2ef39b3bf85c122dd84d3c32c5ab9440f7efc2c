import ErdosProblems.Erdos730.FullDensityTheorem

/-!
An intentionally small correspondence witness. The verification script binds
the two source files and rejects any change to the exact proposition bytes.
-/

namespace LeanCorrespondence.Erdos730

abbrev FormalConjecturesAffirmativeRhs : Prop :=
  {(n, m) : ℕ × ℕ |
      n < m ∧ n.centralBinom.primeFactors = m.centralBinom.primeFactors}.Infinite

abbrev PalomarChallengeProposition : Prop :=
  {(n, m) : ℕ × ℕ |
      n < m ∧ n.centralBinom.primeFactors = m.centralBinom.primeFactors}.Infinite

theorem proposition_identity :
    FormalConjecturesAffirmativeRhs = PalomarChallengeProposition := rfl

theorem affirmative_rhs_witness : FormalConjecturesAffirmativeRhs :=
  Erdos730.FullDensityTheorem.pairSet_infinite

theorem palomar_challenge_witness : PalomarChallengeProposition :=
  Erdos730.FullDensityTheorem.pairSet_infinite

#print axioms affirmative_rhs_witness
#print axioms palomar_challenge_witness

end LeanCorrespondence.Erdos730
