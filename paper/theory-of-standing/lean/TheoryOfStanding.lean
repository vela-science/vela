import Std

/-!
# A minimal executable model of Vela Standing

This file is a proof artifact, not an implementation of the Vela wire protocol.
It models only the semantic distinctions needed for Submission, scoped
Verification, Repository-local Decision admission, Event history, correction,
deterministic replay, and derived Standing.

The finite separation witness is abstract. It does not encode or inspect any
held-out evaluation fixture, and its D replay is proved against the definitions
in this file rather than treated as an oracle.
-/

namespace TheoryOfStanding

abbrev RepositoryId := Nat
abbrev ActorId := Nat
abbrev ClaimId := Nat
abbrev DecisionId := Nat
abbrev Root := Nat
abbrev ResourceId := Nat
abbrev Version := Nat
abbrev ScopeId := Nat

inductive Status where
  | unassessed
  | accepted
  | superseded
  | mustReassess
  deriving DecidableEq, Repr

inductive VerificationOutcome where
  | pass
  | fail
  deriving DecidableEq, Repr

structure Submission where
  claim : ClaimId
  producer : ActorId
  scope : ScopeId
  authenticated : Bool
  deriving DecidableEq, Repr

structure Verification where
  claim : ClaimId
  scope : ScopeId
  property : Nat
  outcome : VerificationOutcome
  deriving DecidableEq, Repr

inductive Action where
  | accept (claim : ClaimId)
  | reject (claim : ClaimId)
  | correct
      (priorDecision : DecisionId)
      (predecessor replacement : ClaimId)
      (consequences : List ClaimId)
  deriving DecidableEq, Repr

structure Decision where
  id : DecisionId
  repository : RepositoryId
  authorityLabel : ActorId
  performer : ActorId
  expectedRoot : Root
  readSet : List (ResourceId × Version)
  action : Action
  deriving DecidableEq, Repr

structure Event where
  decisionId : DecisionId
  repository : RepositoryId
  authorityLabel : ActorId
  performer : ActorId
  action : Action
  deriving DecidableEq, Repr

structure RepositoryConfig where
  repository : RepositoryId
  authorizedPerformers : List ActorId
  deriving DecidableEq, Repr

abbrev Standing := RepositoryId → ClaimId → Status
abbrev Versions := ResourceId → Version

structure State where
  root : Root
  versions : Versions
  standing : Standing
  submissions : List Submission
  verifications : List Verification
  events : List Event

def initialState : State where
  root := 0
  versions := fun _ => 0
  standing := fun _ _ => .unassessed
  submissions := []
  verifications := []
  events := []

def matchingSubmission (s : State) (claim : ClaimId) (scope : ScopeId) : Bool :=
  s.submissions.any fun submission =>
    submission.claim == claim && submission.scope == scope

def submittedClaim (s : State) (claim : ClaimId) : Bool :=
  s.submissions.any fun submission => submission.claim == claim

def passingVerification (s : State) (claim : ClaimId) : Bool :=
  s.verifications.any fun verification =>
    verification.claim == claim && verification.outcome == .pass

def currentReadSet (s : State) (readSet : List (ResourceId × Version)) : Bool :=
  readSet.all fun entry => s.versions entry.1 == entry.2

def eligibleAction (s : State) : Action → Bool
  | .accept claim => submittedClaim s claim && passingVerification s claim
  | .reject claim => submittedClaim s claim
  | .correct _ _ replacement _ =>
      submittedClaim s replacement && passingVerification s replacement

def validCorrectionReference (s : State) (decision : Decision) : Bool :=
  match decision.action with
  | .correct prior predecessor _ _ =>
      (s.events.any fun event =>
        event.decisionId == prior &&
        event.repository == decision.repository &&
        event.action == .accept predecessor) &&
      s.standing decision.repository predecessor == .accepted
  | _ => true

inductive AdmissionError where
  | wrongRepository
  | unauthorized
  | misattributed
  | staleRoot
  | staleReadSet
  | ineligible
  | invalidCorrectionReference
  deriving DecidableEq, Repr

def admissionError
    (config : RepositoryConfig) (s : State) (decision : Decision) :
    Option AdmissionError :=
  if decision.repository ≠ config.repository then
    some .wrongRepository
  else if !config.authorizedPerformers.contains decision.performer then
    some .unauthorized
  else if decision.authorityLabel ≠ decision.performer then
    some .misattributed
  else if decision.expectedRoot ≠ s.root then
    some .staleRoot
  else if !currentReadSet s decision.readSet then
    some .staleReadSet
  else if !eligibleAction s decision.action then
    some .ineligible
  else if !validCorrectionReference s decision then
    some .invalidCorrectionReference
  else
    none

def admissible
    (config : RepositoryConfig) (s : State) (decision : Decision) : Bool :=
  decision.repository == config.repository &&
  config.authorizedPerformers.contains decision.performer &&
  decision.authorityLabel == decision.performer &&
  decision.expectedRoot == s.root &&
  currentReadSet s decision.readSet &&
  eligibleAction s decision.action &&
  validCorrectionReference s decision

def toEvent (decision : Decision) : Event where
  decisionId := decision.id
  repository := decision.repository
  authorityLabel := decision.authorityLabel
  performer := decision.performer
  action := decision.action

def setStatus
    (standing : Standing) (repository : RepositoryId) (claim : ClaimId)
    (status : Status) : Standing :=
  fun otherRepository otherClaim =>
    if otherRepository = repository ∧ otherClaim = claim then
      status
    else
      standing otherRepository otherClaim

def correctionStanding
    (standing : Standing) (repository : RepositoryId)
    (predecessor replacement : ClaimId) (consequences : List ClaimId) : Standing :=
  fun otherRepository claim =>
    if otherRepository ≠ repository then
      standing otherRepository claim
    else if claim = predecessor then
      .superseded
    else if claim = replacement then
      .accepted
    else if claim ∈ consequences then
      .mustReassess
    else
      standing otherRepository claim

def admittedStanding
    (config : RepositoryConfig) (s : State) (decision : Decision) : Standing :=
  match decision.action with
  | .accept claim => setStatus s.standing config.repository claim .accepted
  | .reject _ => s.standing
  | .correct _ predecessor replacement consequences =>
      correctionStanding s.standing config.repository predecessor replacement consequences

def applyAdmitted
    (config : RepositoryConfig) (s : State) (decision : Decision) : State :=
  { s with
    root := s.root + 1
    standing := admittedStanding config s decision
    events := s.events ++ [toEvent decision] }

def applyDecision
    (config : RepositoryConfig) (s : State) (decision : Decision) : State :=
  if admissible config s decision then
    applyAdmitted config s decision
  else
    s

def applySubmission (s : State) (submission : Submission) : State :=
  if submission.authenticated then
    { s with
      root := s.root + 1
      submissions := s.submissions ++ [submission] }
  else
    s

def applyVerification (s : State) (verification : Verification) : State :=
  if matchingSubmission s verification.claim verification.scope then
    { s with
      root := s.root + 1
      verifications := s.verifications ++ [verification] }
  else
    s

inductive Record where
  | submission (value : Submission)
  | verification (value : Verification)
  | decision (value : Decision)
  deriving DecidableEq, Repr

def step (config : RepositoryConfig) (s : State) : Record → State
  | .submission submission => applySubmission s submission
  | .verification verification => applyVerification s verification
  | .decision decision => applyDecision config s decision

def replayFrom
    (config : RepositoryConfig) (initial : State) (history : List Record) : State :=
  history.foldl (step config) initial

def replay (config : RepositoryConfig) (history : List Record) : State :=
  replayFrom config initialState history

def ReplaysTo
    (config : RepositoryConfig) (history : List Record) (result : State) : Prop :=
  replay config history = result

/-! ## General invariants -/

theorem replay_determinism
    {config : RepositoryConfig} {history : List Record} {left right : State}
    (hLeft : ReplaysTo config history left)
    (hRight : ReplaysTo config history right) :
    left = right :=
  hLeft.symm.trans hRight

theorem submission_does_not_change_standing
    (config : RepositoryConfig) (s : State) (submission : Submission) :
    (step config s (.submission submission)).standing = s.standing := by
  change (applySubmission s submission).standing = s.standing
  unfold applySubmission
  split <;> rfl

theorem verification_does_not_change_standing
    (config : RepositoryConfig) (s : State) (verification : Verification) :
    (step config s (.verification verification)).standing = s.standing := by
  change (applyVerification s verification).standing = s.standing
  unfold applyVerification
  split <;> rfl

theorem rejected_decision_fails_closed
    (config : RepositoryConfig) (s : State) (decision : Decision)
    (rejected : admissible config s decision = false) :
    applyDecision config s decision = s := by
  simp [applyDecision, rejected]

theorem unauthorized_decision_fails_closed
    (config : RepositoryConfig) (s : State) (decision : Decision)
    (unauthorized : decision.performer ∉ config.authorizedPerformers) :
    applyDecision config s decision = s := by
  apply rejected_decision_fails_closed
  simp [admissible, unauthorized]

theorem stale_root_decision_fails_closed
    (config : RepositoryConfig) (s : State) (decision : Decision)
    (stale : decision.expectedRoot ≠ s.root) :
    applyDecision config s decision = s := by
  apply rejected_decision_fails_closed
  simp [admissible, stale]

theorem stale_read_set_decision_fails_closed
    (config : RepositoryConfig) (s : State) (decision : Decision)
    (stale : currentReadSet s decision.readSet = false) :
    applyDecision config s decision = s := by
  apply rejected_decision_fails_closed
  simp [admissible, stale]

theorem misattributed_decision_fails_closed
    (config : RepositoryConfig) (s : State) (decision : Decision)
    (misattributed : decision.authorityLabel ≠ decision.performer) :
    applyDecision config s decision = s := by
  apply rejected_decision_fails_closed
  simp [admissible, misattributed]

theorem correction_reference_invalid_fails_closed
    (config : RepositoryConfig) (s : State) (decision : Decision)
    (invalid : validCorrectionReference s decision = false) :
    applyDecision config s decision = s := by
  apply rejected_decision_fails_closed
  simp [admissible, invalid]

theorem no_error_implies_authorized
    (config : RepositoryConfig) (s : State) (decision : Decision)
    (admitted : admissible config s decision = true) :
    decision.performer ∈ config.authorizedPerformers := by
  by_cases authorized : decision.performer ∈ config.authorizedPerformers
  · exact authorized
  · have rejected : admissible config s decision = false := by
      simp [admissible, authorized]
    rw [rejected] at admitted
    contradiction

theorem standing_change_implies_admitted_authorized_decision
    (config : RepositoryConfig) (s : State) (record : Record)
    (changed : (step config s record).standing ≠ s.standing) :
    ∃ decision,
      record = .decision decision ∧
      admissible config s decision = true ∧
      decision.performer ∈ config.authorizedPerformers := by
  cases record with
  | submission submission =>
      exact False.elim (changed (submission_does_not_change_standing config s submission))
  | verification verification =>
      exact False.elim (changed (verification_does_not_change_standing config s verification))
  | decision decision =>
      cases admitted : admissible config s decision with
      | true =>
          exact ⟨decision, rfl, admitted,
            no_error_implies_authorized config s decision admitted⟩
      | false =>
          have unchanged := congrArg State.standing
            (rejected_decision_fails_closed config s decision admitted)
          exact False.elim (changed unchanged)

theorem decision_is_repository_local
    (config : RepositoryConfig) (s : State) (decision : Decision)
    (otherRepository : RepositoryId)
    (different : otherRepository ≠ config.repository) (claim : ClaimId) :
    (applyDecision config s decision).standing otherRepository claim =
      s.standing otherRepository claim := by
  cases admitted : admissible config s decision with
  | false =>
      simp [applyDecision, admitted]
  | true =>
      cases action : decision.action <;>
        simp [applyDecision, admitted, applyAdmitted, admittedStanding, action,
          setStatus, correctionStanding, different]

theorem admitted_decision_appends_exact_event
    (config : RepositoryConfig) (s : State) (decision : Decision)
    (admitted : admissible config s decision = true) :
    (applyDecision config s decision).events = s.events ++ [toEvent decision] := by
  simp [applyDecision, admitted, applyAdmitted]

theorem correction_history_preserved
    (config : RepositoryConfig) (s : State) (decision : Decision)
    (prior : DecisionId) (predecessor replacement : ClaimId)
    (consequences : List ClaimId)
    (isCorrection : decision.action =
      .correct prior predecessor replacement consequences)
    (admitted : admissible config s decision = true) :
    (applyDecision config s decision).events = s.events ++ [toEvent decision] ∧
    (toEvent decision).action =
      .correct prior predecessor replacement consequences := by
  exact ⟨admitted_decision_appends_exact_event config s decision admitted,
    by simpa [toEvent] using isCorrection⟩

theorem correction_predecessor_is_superseded
    (config : RepositoryConfig) (s : State) (decision : Decision)
    (prior : DecisionId) (predecessor replacement : ClaimId)
    (consequences : List ClaimId)
    (isCorrection : decision.action =
      .correct prior predecessor replacement consequences)
    (admitted : admissible config s decision = true) :
    (applyDecision config s decision).standing config.repository predecessor =
      .superseded := by
  simp [applyDecision, admitted, applyAdmitted, admittedStanding,
    isCorrection, correctionStanding]

theorem correction_replacement_is_accepted
    (config : RepositoryConfig) (s : State) (decision : Decision)
    (prior : DecisionId) (predecessor replacement : ClaimId)
    (consequences : List ClaimId)
    (distinct : replacement ≠ predecessor)
    (isCorrection : decision.action =
      .correct prior predecessor replacement consequences)
    (admitted : admissible config s decision = true) :
    (applyDecision config s decision).standing config.repository replacement =
      .accepted := by
  simp [applyDecision, admitted, applyAdmitted, admittedStanding,
    isCorrection, correctionStanding, distinct]

theorem correction_consequence_updates_deterministically
    (config : RepositoryConfig) (s : State) (decision : Decision)
    (prior : DecisionId) (predecessor replacement consequence : ClaimId)
    (consequences : List ClaimId)
    (included : consequence ∈ consequences)
    (notPredecessor : consequence ≠ predecessor)
    (notReplacement : consequence ≠ replacement)
    (isCorrection : decision.action =
      .correct prior predecessor replacement consequences)
    (admitted : admissible config s decision = true) :
    (applyDecision config s decision).standing config.repository consequence =
      .mustReassess := by
  simp [applyDecision, admitted, applyAdmitted, admittedStanding,
    isCorrection, correctionStanding, included, notPredecessor, notReplacement]

/-! ## Concrete authority-local and C-versus-D witnesses -/

def repositoryA : RepositoryConfig :=
  { repository := 1, authorizedPerformers := [101] }

def repositoryB : RepositoryConfig :=
  { repository := 2, authorizedPerformers := [202] }

def portableSubmission : Submission :=
  { claim := 10, producer := 900, scope := 7, authenticated := true }

def portableVerification : Verification :=
  { claim := 10, scope := 7, property := 42, outcome := .pass }

def acceptA : Decision :=
  { id := 1, repository := 1, authorityLabel := 101, performer := 101
    expectedRoot := 2, readSet := [(0, 0)], action := .accept 10 }

def rejectB : Decision :=
  { id := 1, repository := 2, authorityLabel := 202, performer := 202
    expectedRoot := 2, readSet := [(0, 0)], action := .reject 10 }

def authorityAHistory : List Record :=
  [.submission portableSubmission, .verification portableVerification, .decision acceptA]

def authorityBHistory : List Record :=
  [.submission portableSubmission, .verification portableVerification, .decision rejectB]

theorem plural_authority_consistency :
    (replay repositoryA authorityAHistory).standing 1 10 = .accepted ∧
    (replay repositoryB authorityBHistory).standing 2 10 = .unassessed ∧
    (replay repositoryA authorityAHistory).standing 2 10 = .unassessed ∧
    (replay repositoryB authorityBHistory).standing 1 10 = .unassessed := by
  native_decide

def dependentSubmission : Submission :=
  { claim := 20, producer := 900, scope := 7, authenticated := true }

def dependentVerification : Verification :=
  { claim := 20, scope := 7, property := 43, outcome := .pass }

def replacementSubmission : Submission :=
  { claim := 11, producer := 900, scope := 7, authenticated := true }

def replacementVerification : Verification :=
  { claim := 11, scope := 7, property := 44, outcome := .pass }

def acceptDependent : Decision :=
  { id := 2, repository := 1, authorityLabel := 101, performer := 101
    expectedRoot := 5, readSet := [(0, 0)], action := .accept 20 }

def freshCorrection : Decision :=
  { id := 3, repository := 1, authorityLabel := 101, performer := 101
    expectedRoot := 8, readSet := [(0, 0)]
    action := .correct 1 10 11 [20] }

def staleCorrection : Decision :=
  { freshCorrection with expectedRoot := 7 }

def historyPrefix : List Record :=
  [ .submission portableSubmission
  , .verification portableVerification
  , .decision acceptA
  , .submission dependentSubmission
  , .verification dependentVerification
  , .decision acceptDependent
  , .submission replacementSubmission
  , .verification replacementVerification
  ]

def freshHistory : List Record := historyPrefix ++ [.decision freshCorrection]
def staleHistory : List Record := historyPrefix ++ [.decision staleCorrection]

inductive CRecord where
  | submission (value : Submission)
  | verification (value : Verification)
  | labelledStateEvent
      (repository : RepositoryId) (authorityLabel : ActorId) (action : Action)
  deriving DecidableEq, Repr

def cView : Record → CRecord
  | .submission submission => .submission submission
  | .verification verification => .verification verification
  | .decision decision =>
      .labelledStateEvent decision.repository decision.authorityLabel decision.action

def prefixState : State := replay repositoryA historyPrefix

theorem finite_c_versus_d_separation :
    freshHistory.map cView = staleHistory.map cView ∧
    freshHistory ≠ staleHistory ∧
    admissionError repositoryA prefixState freshCorrection = none ∧
    admissionError repositoryA prefixState staleCorrection = some .staleRoot ∧
    (replay repositoryA freshHistory).standing 1 10 = .superseded ∧
    (replay repositoryA freshHistory).standing 1 11 = .accepted ∧
    (replay repositoryA freshHistory).standing 1 20 = .mustReassess ∧
    (replay repositoryA staleHistory).standing 1 10 = .accepted ∧
    (replay repositoryA staleHistory).standing 1 11 = .unassessed ∧
    (replay repositoryA staleHistory).standing 1 20 = .accepted := by
  decide

def unauthorizedCorrection : Decision :=
  { freshCorrection with authorityLabel := 404, performer := 404 }

def staleReadSetCorrection : Decision :=
  { freshCorrection with readSet := [(0, 1)] }

def misattributedCorrection : Decision :=
  { freshCorrection with authorityLabel := 303 }

def invalidReferenceCorrection : Decision :=
  { freshCorrection with action := .correct 999 10 11 [20] }

example : admissionError repositoryA prefixState unauthorizedCorrection =
    some .unauthorized := by native_decide

example : admissionError repositoryA prefixState staleCorrection =
    some .staleRoot := by native_decide

example : admissionError repositoryA prefixState staleReadSetCorrection =
    some .staleReadSet := by native_decide

example : admissionError repositoryA prefixState misattributedCorrection =
    some .misattributed := by native_decide

example : admissionError repositoryA prefixState invalidReferenceCorrection =
    some .invalidCorrectionReference := by native_decide

#eval
  [ admissionError repositoryA prefixState unauthorizedCorrection
  , admissionError repositoryA prefixState staleCorrection
  , admissionError repositoryA prefixState staleReadSetCorrection
  , admissionError repositoryA prefixState misattributedCorrection
  , admissionError repositoryA prefixState invalidReferenceCorrection
  ]

#eval
  ( (replay repositoryA freshHistory).standing 1 10
  , (replay repositoryA freshHistory).standing 1 11
  , (replay repositoryA freshHistory).standing 1 20
  , (replay repositoryA staleHistory).standing 1 10
  , (replay repositoryA staleHistory).standing 1 11
  , (replay repositoryA staleHistory).standing 1 20 )

end TheoryOfStanding
