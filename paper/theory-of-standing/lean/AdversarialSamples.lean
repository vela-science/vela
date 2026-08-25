import TheoryOfStanding

/-!
Finite kernel-checked samples for the Phase III P1.3 adversarial model corpus.
These are model-layer checks only; they introduce no Vela wire objects.
-/

namespace TheoryOfStanding.AdversarialSamples

def retryCorrection : Decision :=
  { freshCorrection with id := 4 }

def wrongRepositoryCorrection : Decision :=
  { freshCorrection with repository := 2 }

def ineligibleCorrection : Decision :=
  { freshCorrection with action := .correct 1 10 99 }

def unauthenticatedSubmission : Submission :=
  { claim := 30, producer := 900, scope := 7, authenticated := false }

def unmatchedVerification : Verification :=
  { claim := 10, scope := 99, property := 42, outcome := .pass }

def secondPerformerDecision : Decision :=
  { acceptA with authorityLabel := 202, performer := 202 }

def twoPerformerRepository : RepositoryConfig :=
  { repository := 1, authorizedPerformers := [101, 202] }

def continued (rejected : Decision) : List Record :=
  historyPrefix ++ [.decision rejected, .decision retryCorrection]

abbrev correctionStandingResult (history : List Record) : Prop :=
  (replay repositoryA history).root = 9 ∧
  (replay repositoryA history).standing 1 10 = .superseded ∧
  (replay repositoryA history).standing 1 11 = .accepted ∧
  (replay repositoryA history).standing 1 20 = .accepted

theorem unauthenticated_submission_is_noop :
    (replay repositoryA [.submission unauthenticatedSubmission]).root = 0 ∧
    (replay repositoryA [.submission unauthenticatedSubmission]).events = [] ∧
    (replay repositoryA [.submission unauthenticatedSubmission]).standing 1 30 =
      .unassessed := by
  decide

theorem unmatched_verification_is_noop :
    (replay repositoryA [.verification unmatchedVerification]).root = 0 ∧
    (replay repositoryA [.verification unmatchedVerification]).events = [] ∧
    (replay repositoryA [.verification unmatchedVerification]).standing 1 10 =
      .unassessed := by
  decide

theorem second_authorized_performer_is_admitted :
    (replay twoPerformerRepository
      [.submission portableSubmission, .verification portableVerification,
       .decision secondPerformerDecision]).standing 1 10 = .accepted := by
  decide

theorem wrong_repository_continues :
    admissionError repositoryA prefixState wrongRepositoryCorrection =
      some .wrongRepository ∧
    correctionStandingResult (continued wrongRepositoryCorrection) := by
  decide

theorem unauthorized_continues :
    admissionError repositoryA prefixState unauthorizedCorrection =
      some .unauthorized ∧
    correctionStandingResult (continued unauthorizedCorrection) := by
  decide

theorem misattributed_continues :
    admissionError repositoryA prefixState misattributedCorrection =
      some .misattributed ∧
    correctionStandingResult (continued misattributedCorrection) := by
  decide

theorem stale_root_continues :
    admissionError repositoryA prefixState staleCorrection = some .staleRoot ∧
    correctionStandingResult (continued staleCorrection) := by
  decide

theorem stale_read_set_continues :
    admissionError repositoryA prefixState staleReadSetCorrection =
      some .staleReadSet ∧
    correctionStandingResult (continued staleReadSetCorrection) := by
  decide

theorem ineligible_continues :
    admissionError repositoryA prefixState ineligibleCorrection = some .ineligible ∧
    correctionStandingResult (continued ineligibleCorrection) := by
  decide

theorem invalid_correction_reference_continues :
    admissionError repositoryA prefixState invalidReferenceCorrection =
      some .invalidCorrectionReference ∧
    correctionStandingResult (continued invalidReferenceCorrection) := by
  decide

def multipleRejectedHistory : List Record :=
  historyPrefix ++
    [.decision staleCorrection, .decision unauthorizedCorrection,
     .decision retryCorrection]

theorem multiple_rejections_are_noops_before_retry :
    correctionStandingResult multipleRejectedHistory := by
  decide

theorem valid_correction_matches_fresh_witness :
    correctionStandingResult freshHistory := by
  decide

theorem plural_authority_sample :
    (replay repositoryA authorityAHistory).standing 1 10 = .accepted ∧
    (replay repositoryB authorityBHistory).standing 2 10 = .unassessed ∧
    (replay repositoryA authorityAHistory).standing 2 10 = .unassessed ∧
    (replay repositoryB authorityBHistory).standing 1 10 = .unassessed :=
  plural_authority_consistency

theorem descriptive_projection_sample :
    (deriveReassessment (replay repositoryA freshHistory)
      descriptiveDependencies 10).reassessment 20 = .needsReassessment ∧
    (deriveReassessment (replay repositoryA freshHistory)
      [] 10).reassessment 20 = .unaffected ∧
    (deriveReassessment (replay repositoryA freshHistory)
      descriptiveDependencies 10).canonicalStanding =
    (deriveReassessment (replay repositoryA freshHistory)
      [] 10).canonicalStanding :=
  descriptive_data_changes_projection_not_standing

end TheoryOfStanding.AdversarialSamples
