/-
Copyright 2025 The Formal Conjectures Authors.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    https://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
-/

import FormalConjecturesUtil

/-!
# Erdős Problem 264

*Reference:* [erdosproblems.com/264](https://www.erdosproblems.com/264)
-/

namespace Erdos264

open Filter

open scoped ENNReal Asymptotics

/--
A sequence $a_n$ of integers is called an irrationality sequence if for every bounded sequence of integers $b_n$ with $a_n + b_n \neq 0$ and
$b_n \neq 0$ for all $n$, the sum
$$
  \sum \frac{1}{a_n + b_n}
$$
is irrational.

Note: there are other possible definitions of this concept. See
FormalConjectures/ErdosProblems/263.lean for another possible definition.
-/
def IsIrrationalitySequence (a : ℕ → ℕ) : Prop := ∀ b : ℕ → ℤ,
  BddAbove (Set.range b) → BddBelow (Set.range b) →
  0 ∉ Set.range (fun n ↦ (a n : ℤ) + b n) → 0 ∉ Set.range b →
  Irrational (∑' n, (1 : ℝ) / ((a n : ℤ) + b n))

/--
Is $2^n$ an example of an irrationality sequence? Kovač and Tao proved that it is not [KoTa24]

[KoTa24] Kovač, V. and Tao T., On several irrationality problems for Ahmes series. arXiv:2406.17593 (2024).
-/
@[category research solved, AMS 11]
theorem erdos_264.parts.i : ¬IsIrrationalitySequence (2 ^ ·) := by
  set_option maxHeartbeats 50000000 in
  exact by
    classical
    have gap_inequality (n : ℕ) (k : ℕ) (h_ge : 1 ≤ k) (h_le : k ≤ 3) :
      1 / (2^n + (k : ℝ)) - 1 / (2^n + (k : ℝ) + 1) ≤
      ∑' m, (1 / (2^(n + 1 + m) + 1 : ℝ) - 1 / (2^(n + 1 + m) + 4 : ℝ)) := by
        -- Let's simplify the general term of the series.
        have h_term :
            ∀ m : ℕ,
              (1 / (2 ^ (n + 1 + m) + 1 : ℝ) -
                  1 / (2 ^ (n + 1 + m) + 4 : ℝ)) =
                3 / ((2 ^ (n + 1 + m) + 1) *
                  (2 ^ (n + 1 + m) + 4) : ℝ) := by
          intro m
          rw [div_sub_div] <;> ring_nf <;> positivity
        -- We'll use the fact that the sum of a geometric series can be bounded.
        have h_geo_series :
            (∑' m : ℕ,
              3 / ((2 ^ (n + 1 + m) + 1) *
                (2 ^ (n + 1 + m) + 4) : ℝ)) ≥
              3 / ((2 ^ (n + 1) + 1) * (2 ^ (n + 1) + 4) : ℝ) *
                ∑' m : ℕ, (1 / 4 : ℝ) ^ m := by
          rw [← tsum_mul_left]
          refine Summable.tsum_le_tsum ?_ ?_ ?_
          · intro i
            field_simp
            norm_num [pow_add, pow_mul]
            rw [
              show (1 / 4 : ℝ) ^ i = ((2 ^ i : ℝ) ^ 2)⁻¹ by
                rw [one_div, inv_pow]
                norm_num [sq, ← mul_pow]
            ]
            field_simp
            nlinarith [
              show ( 2 ^ n : ℝ ) ≥ 1 by exact one_le_pow₀ ( by norm_num ),
              show ( 2 ^ i : ℝ ) ≥ 1 by exact one_le_pow₀ ( by norm_num ),
              mul_le_mul_of_nonneg_left
                ( show ( 2 ^ n : ℝ ) ≥ 1 by exact one_le_pow₀ ( by norm_num ) )
                ( show ( 0 : ℝ ) ≤ 2 ^ i by positivity ),
              mul_le_mul_of_nonneg_left
                ( show ( 2 ^ i : ℝ ) ≥ 1 by exact one_le_pow₀ ( by norm_num ) )
                ( show ( 0 : ℝ ) ≤ 2 ^ n by positivity )
            ]
          · exact Summable.mul_left _ ( summable_geometric_of_lt_one ( by norm_num ) ( by norm_num ) )
          · -- We can compare our series to a geometric series with ratio $1/4$.
            have h_compare :
                ∀ m : ℕ,
                  (3 / ((2 ^ (n + 1 + m) + 1) *
                    (2 ^ (n + 1 + m) + 4) : ℝ)) ≤
                    3 / (4 ^ (n + 1 + m) : ℝ) := by
              intro m
              rw [ div_le_div_iff₀ ] <;> norm_cast <;> ring_nf <;> norm_num
              norm_num [ pow_mul', ← mul_pow ]
            exact Summable.of_nonneg_of_le ( fun m => by positivity ) h_compare ( by
              exact Summable.mul_left _ <| by
                simpa using summable_geometric_of_lt_one ( by norm_num )
                  ( inv_lt_one_of_one_lt₀ <| by norm_num )
                  |> Summable.comp_injective <| by
                    intro m
                    aesop );
        have h_rhs_lower :
            4 / (((2 : ℝ) ^ (n + 1) + 1) * ((2 : ℝ) ^ (n + 1) + 4)) ≤
              ∑' m, (1 / (2^(n + 1 + m) + 1 : ℝ) - 1 / (2^(n + 1 + m) + 4 : ℝ)) := by
          calc
            4 / (((2 : ℝ) ^ (n + 1) + 1) * ((2 : ℝ) ^ (n + 1) + 4))
                = 3 / (((2 : ℝ) ^ (n + 1) + 1) * ((2 : ℝ) ^ (n + 1) + 4)) *
                  (∑' m : ℕ, (1 / 4 : ℝ) ^ m) := by
                    norm_num [tsum_geometric_of_lt_one]
            _ ≤ ∑' m : ℕ, (3 / ((2 ^ (n + 1 + m) + 1) *
                  (2 ^ (n + 1 + m) + 4) : ℝ)) := by
                    simpa using h_geo_series
            _ = ∑' m, (1 / (2^(n + 1 + m) + 1 : ℝ) -
                  1 / (2^(n + 1 + m) + 4 : ℝ)) := by
                    exact tsum_congr fun m => (h_term m).symm
        refine le_trans ?_ h_rhs_lower
        interval_cases k <;> norm_num [pow_succ]
        · field_simp
          nlinarith [pow_pos (by positivity : (0 : ℝ) < 2) n]
        · field_simp
          nlinarith [pow_pos (by positivity : (0 : ℝ) < 2) n]
        · field_simp
          nlinarith [pow_pos (by positivity : (0 : ℝ) < 2) n]

    /-
    The set of sums S.
    -/

    /-
    Inductive step: if z is in the range of tail sums at step n, we can choose
    the next term b_n such that the remainder is in the range of tail sums at
    step n+1.
    -/
    let min_tail (n : ℕ) : ℝ := ∑' m, 1 / (2^(n + m) + 4 : ℝ)
    let max_tail (n : ℕ) : ℝ := ∑' m, 1 / (2^(n + m) + 1 : ℝ)

    -- The two tail intervals have nonempty interior.
    have h_min_lt_max : min_tail 0 < max_tail 0 := by
      change (∑' n, 1 / (2 ^ (0 + n) + 4 : ℝ)) < ∑' n, 1 / (2 ^ (0 + n) + 1 : ℝ)
      refine Summable.tsum_lt_tsum (i := 0) ?_ ?_ ?_ ?_
      · intro n
        exact one_div_le_one_div_of_le (by positivity) (by linarith)
      · norm_num
      · -- Compare with the convergent geometric series.
        have h_comp : ∀ n : ℕ, (1 : ℝ) / (2 ^ n + 4) ≤ 1 / 2 ^ n := by
          exact fun n => by gcongr; norm_num;
        exact Summable.of_nonneg_of_le
          ( fun n => by positivity )
          ( fun n => by simpa using h_comp n )
          ( by simpa using summable_geometric_two );
      · -- Compare with the same geometric series.
        have h_comparison : ∀ n : ℕ, (1 : ℝ) / (2 ^ n + 1) ≤ (1 / 2) ^ n := by
          -- By simplifying, we can see that $1/(2^n + 1) \leq 1/2^n$ for all $n$.
          intro n
          field_simp;
          ring_nf; norm_num [ pow_mul', ← mul_pow ] ;
        exact Summable.of_nonneg_of_le
          ( fun n => by positivity )
          ( fun n => by simpa using h_comparison n )
          ( summable_geometric_two );

    have inductive_step (n : ℕ) (z : ℝ) (hz : z ∈ Set.Icc (min_tail n) (max_tail n)) :
      ∃ (k : ℕ), k ∈ Finset.Icc 1 4 ∧
        z - 1 / (2^n + (k : ℝ)) ∈
          Set.Icc (min_tail (n + 1)) (max_tail (n + 1)) := by
        have h_gap_ineq :
            ∀ k ∈ ({1, 2, 3} : Finset ℕ),
              1 / (2 ^ n + (k : ℝ)) - 1 / (2 ^ n + (k : ℝ) + 1) ≤
                max_tail (n + 1) - min_tail (n + 1) := by
          -- Their difference is exactly the series starting from `n+1`.
          have h_diff :
              max_tail (n + 1) - min_tail (n + 1) =
                ∑' m, (1 / (2^(n + 1 + m) + 1 : ℝ) -
                  1 / (2^(n + 1 + m) + 4 : ℝ)) := by
            unfold max_tail min_tail
            rw [ Summable.tsum_sub ]
            · -- This is a convergent geometric-series tail.
              have h_geo_series : Summable (fun m : ℕ => (1 : ℝ) / 2^(n+1+m)) := by
                simpa using summable_geometric_two.comp_injective ( add_right_injective _ );
              exact Summable.of_nonneg_of_le
                ( fun m => by positivity )
                ( fun m => by gcongr ; norm_num )
                h_geo_series
            · -- This is a convergent geometric-series tail.
              have h_geo_series : Summable (fun m : ℕ => (1 : ℝ) / (2^(n + 1 + m))) := by
                simpa using summable_geometric_two.comp_injective ( add_right_injective _ );
              exact Summable.of_nonneg_of_le
                ( fun m => by positivity )
                ( fun m => by gcongr ; norm_num )
                h_geo_series
          exact fun k hk =>
            h_diff ▸ gap_inequality n k
              ( by fin_cases hk <;> norm_num )
              ( by fin_cases hk <;> norm_num ) |>
                le_trans ( by norm_num )
        -- These are the one-term tail decompositions.
        have h_bounds :
            min_tail n = 1 / (2^n + 4 : ℝ) + min_tail (n + 1) ∧
              max_tail n = 1 / (2^n + 1 : ℝ) + max_tail (n + 1) := by
          unfold min_tail max_tail
          constructor
          · rw [Summable.tsum_eq_zero_add]
            · ac_rfl
            · exact Summable.of_nonneg_of_le
                ( fun _ => by positivity )
                ( fun m => by
                  simpa using inv_anti₀ ( by positivity )
                    ( show ( 2 ^ ( n + m ) + 4 : ℝ ) ≥ 2 ^ ( n + m ) by linarith ) )
                ( by
                  simpa using summable_geometric_two.comp_injective ( add_right_injective n ) )
          · rw [Summable.tsum_eq_zero_add]
            · ac_rfl
            · exact Summable.of_nonneg_of_le
                ( fun m => by positivity )
                ( fun m => by
                  simpa using inv_anti₀ ( by positivity )
                    ( show ( 2 ^ ( n + m ) + 1 : ℝ ) ≥ 2 ^ ( n + m ) by norm_num ) )
                ( by
                  simpa using summable_geometric_two.comp_injective ( add_right_injective n ) )
        norm_num [add_assoc] at *
        exact ⟨
          if ( 2 ^ n + 1 : ℝ ) ⁻¹ + min_tail ( n + 1 ) ≤ z then 1
          else if ( 2 ^ n + 2 : ℝ ) ⁻¹ + min_tail ( n + 1 ) ≤ z then 2
          else if ( 2 ^ n + 3 : ℝ ) ⁻¹ + min_tail ( n + 1 ) ≤ z then 3
          else 4,
          by split_ifs <;> norm_num,
          by split_ifs <;> push_cast <;> linarith,
          by split_ifs <;> push_cast <;> linarith ⟩


    /-
    The interval [min_tail 0, max_tail 0] is a subset of SumSet.
    -/
    have Icc_subset_SumSet : ∀ z, z ∈ Set.Icc (min_tail 0) (max_tail 0) →
        ∃ b : ℕ → ℕ, (∀ n, 1 ≤ b n ∧ b n ≤ 4) ∧
          z = ∑' n, (1 : ℝ) / (2 ^ n + b n) := by
      intro z hz;
      -- Construct `b_n` and remainders `z_n` inside the tail intervals.
      have h_seq :
          ∃ b : ℕ → ℕ, (∀ n, 1 ≤ b n ∧ b n ≤ 4) ∧
            ∃ z_seq : ℕ → ℝ,
              (∀ n, z_seq n ∈ Set.Icc (min_tail n) (max_tail n)) ∧
                z_seq 0 = z ∧
                  ∀ n, z_seq (n + 1) = z_seq n - (1 : ℝ) / (2^n + b n) := by
        choose! b hb using fun n z hz => inductive_step n z hz;
        use fun n =>
          b n (Nat.recOn n z fun n ih => ih - 1 / (2 ^ n + (b n ih : ℝ)));
        refine ⟨ ?_, ?_ ⟩
        · intro n;
          have h_seq :
              ∀ n,
                (Nat.recOn n z fun n ih => ih - 1 / (2 ^ n + (b n ih : ℝ))) ∈
                  Set.Icc (min_tail n) (max_tail n) := by
            intro n; induction n <;> aesop;
          exact Finset.mem_Icc.mp ( hb n _ ( h_seq n ) |>.1 );
        · use fun n => Nat.recOn n z fun n ih => ih - 1 / ( 2 ^ n + ( b n ih : ℝ ) );
          exact ⟨ fun n => Nat.recOn n hz fun n ih => hb n _ ih |>.2, rfl, fun n => rfl ⟩;
      obtain ⟨ b, hb, z_seq, hz_seq, rfl, hz_seq' ⟩ := h_seq; use b; aesop;
      -- We need to show $z_n \to 0$ as $n \to \infty$.
      have h_zero : Filter.Tendsto z_seq Filter.atTop (nhds 0) := by
        -- Both tails are bounded by a geometric series tending to 0.
        have h_tail_bound : ∀ n, max_tail n ≤ 2 / 2^n ∧ min_tail n ≥ 0 := by
          intros n
          have h_tail_bound : max_tail n ≤ ∑' m, (1 : ℝ) / (2^(n + m)) := by
            refine Summable.tsum_le_tsum ?_ ?_ ?_
            · exact fun i => by gcongr ; norm_num;
            · exact Summable.of_nonneg_of_le
                ( fun m => by positivity )
                ( fun m => by
                  simpa using inv_anti₀ ( by positivity )
                    ( show ( 2 ^ ( n + m ) + 1 : ℝ ) ≥ 2 ^ ( n + m ) by linarith ) )
                ( by
                  simpa using summable_geometric_two.comp_injective ( add_right_injective n ) );
            · simpa using summable_geometric_two.comp_injective ( add_right_injective n );
          norm_num [ pow_add, tsum_mul_left ] at *;
          exact ⟨
            h_tail_bound.trans <| by
              rw [
                tsum_mul_right,
                show ( ∑' m : ℕ, ( 2 ^ m : ℝ ) ⁻¹ ) = 2 by
                  simpa using tsum_geometric_two
              ];
              ring_nf;
              norm_num,
            tsum_nonneg fun _ => by positivity ⟩;
        exact squeeze_zero
          ( fun n => (h_tail_bound n).2.trans (show min_tail n ≤ z_seq n from by
            simpa [min_tail] using (hz_seq n).1) )
          ( fun n => (show z_seq n ≤ max_tail n from by
            simpa [max_tail] using (hz_seq n).2).trans (h_tail_bound n).1 )
          ( tendsto_const_nhds.div_atTop
            ( tendsto_pow_atTop_atTop_of_one_lt one_lt_two ) );
      -- By definition of $z_seq$, we have $z_seq 0 = \sum_{i=0}^{n-1} \frac{1}{2^i+b_i} + z_seq n$.
      have h_sum : ∀ n, z_seq 0 = ∑ i ∈ Finset.range n, (1 : ℝ) / (2^i + b i) + z_seq n := by
        exact fun n =>
          Nat.recOn n ( by norm_num ) fun n ih => by
            rw [ Finset.sum_range_succ, hz_seq' ];
            norm_num at *;
            linarith;
      -- By definition of $z_seq$, we have $z_seq 0 = \sum_{i=0}^{\infty} \frac{1}{2^i+b_i}$.
      have h_sum_inf :
          Filter.Tendsto (fun n => ∑ i ∈ Finset.range n, (1 : ℝ) / (2^i + b i))
            Filter.atTop (nhds (∑' i, (1 : ℝ) / (2^i + b i))) := by
        exact ( Summable.hasSum <| by
          exact Summable.of_nonneg_of_le
            ( fun i => by positivity )
            ( fun i => by
              simpa using inv_anti₀ ( by positivity ) <| show ( 2 ^ i + b i : ℝ ) ≥ 2 ^ i by
                norm_cast
                linarith [ hb i ] )
            summable_geometric_two ) |>
              HasSum.tendsto_sum_nat;
      have h_sum_inf_zero :
          Filter.Tendsto (fun n => ∑ i ∈ Finset.range n, (1 : ℝ) / (2 ^ i + b i) + z_seq n)
            Filter.atTop (nhds (∑' i, (1 : ℝ) / (2 ^ i + b i))) := by
        simpa only [add_zero] using h_sum_inf.add h_zero
      simpa only [one_div] using tendsto_nhds_unique
        (tendsto_const_nhds.congr fun n => by rw [h_sum n]) h_sum_inf_zero

    /-
    There exists a bounded sequence of positive integers b_n such that the sum of
    1/(2^n + b_n) is rational.
    -/
    have exists_bounded_seq_rational_sum :
      ∃ (b : ℕ → ℕ) (q : ℚ), (∀ n, 1 ≤ b n) ∧ BddAbove (Set.range b) ∧
      ∑' n, (1 : ℝ) / (2^n + b n) = q := by
        obtain ⟨q, hq⟩ := exists_rat_btwn h_min_lt_max
        obtain ⟨b, hb, hsum⟩ := Icc_subset_SumSet q ⟨hq.1.le, hq.2.le⟩
        refine ⟨b, q, fun n => (hb n).1, ?_, ?_⟩
        · exact ⟨4, Set.forall_mem_range.mpr fun n => (hb n).2⟩
        · exact hsum.symm
    obtain ⟨b, q, hb, hbdd, hq⟩ := exists_bounded_seq_rational_sum
    intro h
    unfold IsIrrationalitySequence at h
    have hb_above : BddAbove (Set.range fun n ↦ (b n : ℤ)) := by
      refine ⟨(hbdd.choose : ℤ), ?_⟩
      rintro _ ⟨n, rfl⟩
      change (b n : ℤ) ≤ (hbdd.choose : ℤ)
      exact_mod_cast hbdd.choose_spec (Set.mem_range_self n)
    have hb_below : BddBelow (Set.range fun n ↦ (b n : ℤ)) := by
      refine ⟨1, ?_⟩
      rintro _ ⟨n, rfl⟩
      change (1 : ℤ) ≤ (b n : ℤ)
      exact_mod_cast hb n
    have hab_ne_zero : 0 ∉ Set.range (fun n ↦ ((2 ^ n : ℕ) : ℤ) + (b n : ℤ)) := by
      rintro ⟨n, hn⟩
      have hpos : 0 < ((2 ^ n : ℕ) : ℤ) + (b n : ℤ) := by
        exact add_pos_of_pos_of_nonneg (by exact_mod_cast pow_pos (by norm_num : 0 < (2 : ℕ)) n)
          (by exact_mod_cast Nat.zero_le (b n))
      exact ne_of_gt hpos hn
    have hb_ne_zero : 0 ∉ Set.range (fun n ↦ (b n : ℤ)) := by
      rintro ⟨n, hn⟩
      have hpos : 0 < (b n : ℤ) := by exact_mod_cast hb n
      exact ne_of_gt hpos hn
    have hirr := h (fun n ↦ (b n : ℤ)) hb_above hb_below hab_ne_zero hb_ne_zero
    apply Rat.not_irrational q
    convert hirr using 1
    simpa using hq.symm


/--
Is $n!$ an example of an irrationality sequence?
-/
@[category research open, AMS 11]
theorem erdos_264.parts.ii : answer(sorry) ↔ IsIrrationalitySequence Nat.factorial := by sorry

/--
One example is $2^{2^n}$.
-/
@[category research solved, AMS 11]
theorem erdos_264.variants.example : IsIrrationalitySequence (fun n ↦ 2 ^ (2 ^ n)) := by sorry

/--
Kovač and Tao [KoTa24] generally proved that any strictly increasing sequence of positive integers
$a_n$ such that $\sum \frac{1}{a_n}$ converges and
$$
  \liminf_{n \to \infty} (a_n^2 \sum_{k > n} \frac{1}{a_k^2}) > 0
$$
is not an irrationality sequence.

[KoTa24] Kovač, V. and Tao T., On several irrationality problems for Ahmes series. arXiv:2406.17593 (2024).
-/
@[category research solved, AMS 11]
theorem erdos_264.variants.ko_tao_neg {a : ℕ → ℕ} (h₁ : StrictMono a) (h₂ : 0 ∉ Set.range a)
    (h₃ : Summable ((1 : ℝ) / a ·))
    (h₄ : 0 < atTop.liminf fun n ↦ a n ^ 2 * ∑' k : Set.Ioi n, (1 : ℝ) / a k ^ 2) :
    ¬IsIrrationalitySequence a := by
  sorry

/--
On the other hand, Kovač and Tao [KoTa24] do prove that for any function $F$ with
$\lim_{n \to \infty} \frac{F(n + 1)}{F(n)} = \infty$ there exists such an irrationality sequence with $a_n \sim F(n)$.

[KoTa24] Kovač, V. and Tao T., On several irrationality problems for Ahmes series. arXiv:2406.17593 (2024).
-/
@[category research solved, AMS 11]
theorem erdos_264.variants.ko_tao_pos {F : ℕ → ℕ}
    (hF : atTop.Tendsto (fun n ↦ (F (n + 1) : ℝ) / F n) atTop) :
    ∃ a : ℕ → ℕ, IsIrrationalitySequence a ∧ (fun n ↦ (a n : ℝ)) ~[atTop] fun n ↦ (F n : ℝ) := by
  sorry

end Erdos264
