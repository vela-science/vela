import ErdosProblems

/-!
# Axiom audit

`#print axioms` for every solved-proof target advertised in `proofs.yaml`.
scripts/check_axioms.sh fails the build if any reports `sorryAx` or an
axiom outside `[propext, Classical.choice, Quot.sound]`.
-/

-- Manifest-tracked formal proof targets.
#print axioms Erdos730.FullDensityTheorem.pairSet_infinite
#print axioms Erdos154.erdos_154_sumset
#print axioms Erdos399.erdos_399.variants.cambie
#print axioms Erdos1074.erdos_1074.variants.EHSNumbers_init
#print axioms Erdos94.variants.sum_multiplicity
#print axioms Erdos730.eight_mul_sq_le_succ_cube
#print axioms Erdos730.nearEnvelope_forces_high_exponent
#print axioms Erdos730.nearEnvelope_prime_power_clearance
#print axioms Erdos730.powered_threshold_of_near_maximal
#print axioms Erdos730.finite_prime_power_pair_count
#print axioms Erdos730.finite_reciprocal_tail_from_root_envelopes
#print axioms Erdos730.finite_geometric_prime_power_tail
#print axioms Erdos730.finite_reciprocal_square_tail
#print axioms Erdos730.dyadicThresholdBase_strictMono_step
#print axioms Erdos730.endpoint_powered_threshold_certificate
#print axioms Erdos730.endpoint_payment_identity
#print axioms Erdos730.endpoint_payment_lt_one_percent
#print axioms Erdos730.halfBandEnvelope_forces_high_exponent
#print axioms Erdos730.halfBandEnvelope_prime_power_clearance
#print axioms Erdos730.powered_threshold_of_halfBand_maximal
#print axioms Erdos730.halfBand_endpoint_powered_threshold_certificate
#print axioms Erdos730.halfBand_endpoint_payment_lt_one_percent
#print axioms Erdos730.unitBandEnvelope_forces_high_exponent
#print axioms Erdos730.unitBandEnvelope_prime_power_clearance
#print axioms Erdos730.cutoff_lt_of_unitBand_maximal
#print axioms Erdos730.unitBandDyadicThresholdBase_strictMono_step
#print axioms Erdos730.unitBand_endpoint_threshold_certificate
#print axioms Erdos730.unitBand_endpoint_sqrt_floor_certificate
#print axioms Erdos730.unitBand_endpoint_cuberoot_floor_certificate
#print axioms Erdos730.unitBand_endpoint_payment_identity
#print axioms Erdos730.unitBand_endpoint_payment_lt_one_percent
#print axioms Erdos730.unitBand_endpoint_payment_margin
#print axioms Erdos730.audit_unitBandEnvelope_iff
#print axioms Erdos730.audit_unitBand_complement_iff
#print axioms Erdos730.audit_unitBand_slack_ge_iff
#print axioms Erdos730.UnitRangeBlock.quadratic_block_expansion
#print axioms Erdos730.UnitRangeBlock.quadratic_block_difference_dvd_sq
#print axioms Erdos730.UnitRangeBlock.normalized_block_cover_cross_bound
#print axioms Erdos730.UnitRangeBlock.higher_prime_power_payment_ceiling_lt_half
#print axioms Erdos730.FirstPowerRoutes.first_power_fixed_upper_slope
#print axioms Erdos730.FirstPowerRoutes.normalized_block_cover_six_fifths
#print axioms Erdos730.FirstPowerRoutes.q_top_low_digit_large
#print axioms Erdos730.FirstPowerRoutes.s_top_low_digit_large
#print axioms Erdos730.FirstPowerRoutes.q_top_small_cofactor_of_square_lt
#print axioms Erdos730.FirstPowerRoutes.s_top_small_cofactor_of_square_lt
#print axioms Erdos730.FirstPowerRoutes.q_top_two_digit_large
#print axioms Erdos730.FirstPowerRoutes.s_top_two_digit_large
#print axioms Erdos730.FirstPowerRoutes.improved_higher_power_ceiling_lt_three_tenths
#print axioms Erdos730.FirstPowerRoutes.remaining_first_power_budget
#print axioms Erdos730.KummerTransition.not_dvd_centralBinom_iff_lowerHalfDigits
#print axioms Erdos730.FullDensityCore.product_identity
#print axioms Erdos730.FullDensityCore.branches_pairwise_coprime
#print axioms Erdos730.FullDensityCore.switching_allowed_class_count_certificate
#print axioms Erdos730.padicBranchMap_bijective
#print axioms Erdos730.restrictedDigitBox_card
#print axioms Erdos730.densityBudget_final_lt
#print axioms Erdos730.halfDigitParity_probabilityError
#print axioms Erdos730.padicBranchAllowedCount_le
#print axioms Erdos730.ObstructionMaps.PhiP_root_progression
#print axioms Erdos730.ObstructionMaps.PhiQ_root_progression
#print axioms Erdos730.ObstructionMaps.PhiR_root_progression
#print axioms Erdos730.ObstructionMaps.PhiS_root_progression
#print axioms Erdos730.ObstructionMaps.prime_dvd_residual_support
#print axioms Erdos730.higherPrimePowerPairs_card_le
#print axioms Erdos730.ConsecutiveTransition.consecutive_primeFactors_eq_iff_transitionConditions
#print axioms Erdos730.ConsecutiveTransition.exists_obstruction_of_primeFactors_ne
#print axioms Erdos730.tendsto_tsum_higherPower_of_dominated
#print axioms Erdos730.tendsto_higherPrimePowerPairs_card_div
#print axioms Erdos730.FullDensityTheorem.pairSet_infinite

-- Conditional research surfaces (excluded from the solved-proof manifest).
