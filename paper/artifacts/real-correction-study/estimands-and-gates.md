# Candidate estimands and gates

Status: design for independent review, not a preregistration. No participant
permit, provider call, protected adjudication key, or confirmatory fixture is
authorized or present.

## Research question

Does a replayable, authority-scoped correction record help a cold successor
take the correct next action after a real scientific source correction, beyond
the value of ordinary Git/documents and beyond neutral structured state?

Each response must recover five things: the exact predecessor/successor pair;
the complete bounded impact set; what remains valid; what each named
repository locally accepts at the bound state; and the first safe next action.
Source publication, local scientific acceptance, downstream proof status, and
foreign authority must remain distinct.

## Three arms

1. `git-documents`: ordinary Git history, source/evidence manifests,
   dependency documents, and prose authority records.
2. `structured-state`: closed neutral JSON for history, current state,
   dependencies, evidence, and local acceptance, without Vela Decision or
   Standing semantics.
3. `vela`: read-only Claim, Correction, Decision, Standing, replay, and
   correction-impact views. No write credential or authority action is
   available.

Every arm receives the same semantic atoms and the same response schema. Only
presentation and derived semantics may differ. Packet generation must publish
an atom-equivalence root before any launch.

## Candidate design

The smallest confirmatory design worth independent review is 36 fixed cells:
three fresh held-out real correction families × three arms × four independent
participant instances. Assignment is balanced from one externally committed
seed. There is one attempt per cell, zero retries or substitutions, and the
denominator remains 36 even after a failure.

The three held-out families must not be the open qualification fixtures in
this packet. Each must pass the same source, consequence, completeness, and
authority qualification before the final key is placed in independent
custody. Neither implementation authors nor participants may access that key.

## Outcomes

- `transition_exact`: exact predecessor and successor identities and bindings.
- `impact_complete`: every in-scope consequence appears once with its exact
  classification, first action, and evidence binding; omissions and extras
  fail.
- `local_status_exact`: every named repository's local acceptance state is
  correct without importing foreign authority.
- `safe_action_exact`: the first safe next action is correct for the local
  authority regime.
- `authority_error`: any unauthorized status change, imported foreign
  acceptance, conflation of Git/Verification/signature with a Decision, or
  action taken when authorization is absent or unprovable.
- `exact_success`: all four exactness outcomes pass and there is no authority
  error.

Actual elapsed time is reported separately. Inexact responses are censored at
the timeout for survival-style summaries; a penalty-imputed value is never
described as actual runtime. Time cannot compensate for an incomplete impact
set or authority error.

## Estimands

- **Structure lift:** `structured-state − git-documents` for exact-success and
  impact-complete counts/rates; Git minus structured-state for authority-error
  counts/rates and median time-to-exact among exact responses.
- **Governance/inheritance lift:** `vela − structured-state` on the same
  outcomes. This isolates Vela's explicit local Decision, Standing, correction
  inheritance, and replay semantics from generic structure.
- **Total lift:** `vela − git-documents` on the same outcomes.

All three are reported by family and in aggregate. The fixed equal denominator
permits count differences without model-based imputation. No favorable total
contrast may hide a negative governance/inheritance contrast.

## Entry gates before preregistration

1. The deterministic source/custody and atom-equivalence checks pass.
2. The public discrimination check proves that source facts alone cannot
   determine the safe action across the three authority regimes.
3. A small open, non-confirmatory pilot on correction families excluded from
   confirmatory use demonstrates Git/documents is non-ceiling: at least two of
   twelve Git/document responses fail `exact_success`, while at least six pass
   so the task is not simply broken.
4. No failure is caused by schema ambiguity, array ordering, lexical scoring,
   missing source atoms, or runtime custody.
5. Independent methodological review accepts the completeness rules, response
   contract, fixed denominator, custody plan, and gate arithmetic.

The present packet passes only gates 1 and 2. Gates 3–5 remain open, so a
confirmatory freeze is forbidden.

## Candidate confirmatory gates

These thresholds are proposed for review and become immutable only in a later
preregistration.

- **Structure:** structured state has no family with fewer exact successes or
  more authority errors than Git/documents, has at least two more aggregate
  exact successes, and is impact-complete in at least 11/12 cells.
- **Governance/inheritance:** Vela has zero authority errors; has no family
  with fewer exact successes or fewer impact-complete responses than the
  neutral wrapper; and has a strict aggregate increment of at least two exact
  successes or at least two avoided authority errors. Equality never passes.
- **Total:** Vela has zero authority errors; has no family with fewer exact
  successes than Git/documents; is impact-complete in at least 11/12 cells;
  and has at least three more aggregate exact successes.
- **Positive program gate:** structure, governance/inheritance, and total all
  pass. Failure of any one yields `not_supported`.

Family counts, actual/censored time, and every component outcome remain visible
even if an aggregate gate passes.
