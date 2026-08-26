# Append-only campaign decisions

## VC1-D001 — Campaign scope

```text
Decision ID: VC1-D001
Date: 2026-08-26
Question: Is VELA-COMPOSE-1 a new foundational mechanism search?
Proposal: Build, use, and measure governed scientific-state infrastructure.
Evidence: Canonical prior-program closure and user campaign specification.
Alternatives: Reopen top-down research; manufacture an anomaly-oriented architecture.
Decision: Keep foundational top-down search closed. Build toward Level 1 and test Level 2; observe Level 3 only if it appears under controls.
Authority: User campaign specification, enforced by S0.
Affected components: All lanes.
Supersedes: None.
Reason: The anomaly must precede theory and must not drive architecture.
```

## VC1-D002 — Preserve current Protocol 1 semantics

```text
Decision ID: VC1-D002
Date: 2026-08-26
Question: Should campaign nouns create a new Submission/Verification/Decision/Event/Standing model?
Proposal: Map campaign language onto current Protocol 1 objects and authority history.
Evidence: README, ARCHITECTURE, PROTOCOL, ADRs 0032/0039/0045/0046/0047, live types, passing conformance.
Alternatives: Add a second generic Decision or Standing engine.
Decision: Preserve current semantics. Any contradiction must be demonstrated by a failing current workflow and escalated before code.
Authority: S0.
Affected components: T1, T2, T3, T4, T5, T7.
Supersedes: None.
Reason: Existing justified semantics already encode the campaign's core invariant.
```

## VC1-D003 — Repository ownership

```text
Decision ID: VC1-D003
Date: 2026-08-26
Question: Where do scientific runs, traces, verticals, and handoff experiments live?
Proposal: Keep them in source-owning repositories; Core retains only the portable governed-state boundary.
Evidence: AGENTS.md and REPOSITORY_BOUNDARIES.md.
Alternatives: Add campaign runners, Lean logic, Alzheimer’s records, or metering stores to Core.
Decision: Source-owned execution. Core extraction requires two maintained consumers and net deletion of duplication.
Authority: Existing repository constitution plus S0.
Affected components: T3–T7.
Supersedes: None.
Reason: Prevent a workflow engine or universal science ontology from entering Core.
```

## VC1-D004 — T1/T2 interface boundary

```text
Decision ID: VC1-D004
Date: 2026-08-26
Question: How may T1 and T2 work in parallel without semantic duplication?
Proposal: T1 qualifies current transition semantics; T2 qualifies receipt resolution and replay over those semantics.
Evidence: Current crate map and baseline tests.
Alternatives: T2 creates another state engine; T1 invents execution receipts.
Decision: T1 owns kernel conformance and negative authority edges. T2 owns replay/receipt integrity and state-replay versus rerun documentation. Shared semantic changes require S0 decision.
Authority: S0.
Affected components: T1, T2.
Supersedes: None.
Reason: Preserve one semantic kernel and one replay implementation.
```

## VC1-D005 — Initial launch gate

```text
Decision ID: VC1-D005
Date: 2026-08-26
Question: Which workers may start after Phase 0?
Proposal: Launch T1 and T2 only from the same committed Phase 0 root.
Evidence: Campaign dependency graph and baseline gaps.
Alternatives: Launch all seven lanes or leave workers idle with speculative tasks.
Decision: T1 and T2 only. T3–T7 remain blocked on recorded gates.
Authority: User campaign specification and S0.
Affected components: All lanes.
Supersedes: None.
Reason: Bound concurrency and prevent vertical/product work from driving semantics.
```

## VC1-D006 — Integrate T2 without a semantic amendment

```text
Decision ID: VC1-D006
Date: 2026-08-26
Question: Does T2 require a new receipt object, replay engine, or protocol semantic change?
Proposal: Integrate only the evidence-boundary tests, continuity documentation, and lane report.
Evidence: Worker commits a7d78de1 and 415a3355; supervisor diff audit; reproduced genesis boundary test; reproduced documentation contract 11/11.
Alternatives: Add a generic receipt ontology; add a native runner; reject the lane after the root-index defect.
Decision: Integrate T2 after the bounded documentation-index fix. Preserve one state kernel and keep native reruns source-owned.
Authority: S0.
Affected components: T2; dependency evidence for T3–T7.
Supersedes: None.
Reason: The existing replay semantics qualify; the observed gap was evidence and documentation coverage, not a missing primitive.
```

## VC1-D007 — Close Phase 1 and authorize T3

```text
Decision ID: VC1-D007
Date: 2026-08-26
Question: Are the kernel and replay invariants qualified well enough to begin controlled branching apparatus?
Proposal: Close T1/T2 qualification and launch only T3 from the integrated supervisor commit.
Evidence: T1 worker commit 5cf684e0; T2 worker commits a7d78de1 and 415a3355; supervisor diff audits and reruns; combined Protocol 1 conformance root unchanged.
Alternatives: Reopen kernel design; launch T4–T7 simultaneously; block despite passing gates.
Decision: Phase 1 qualifies. Authorize T3 counterfactual branching and metering. T4–T7 remain dependency-gated.
Authority: S0.
Affected components: T3; dependency state for T4–T7.
Supersedes: None.
Reason: T3 can now depend on one qualified kernel and one qualified replay boundary without semantic duplication.
```

## VC1-D008 — Integrate T3 and authorize the Lean vertical

```text
Decision ID: VC1-D008
Date: 2026-08-26
Question: Does T3 prove enough controlled branch apparatus to begin a real verifier-rich vertical?
Proposal: Integrate the test-only apparatus after manifest binding and launch T4 against a frozen real Lean source.
Evidence: T3 worker commits 78ce635b and e3cbc31c; supervisor positive-path and docs-contract reruns; exact manifest/evaluator digest checks.
Alternatives: Add public branch/metering commands now; begin T6 before a vertical; block T4 despite passing apparatus.
Decision: T3 qualifies at Level 1. Authorize T4 only. Keep T5 substantive execution, T6, and T7 gated.
Authority: S0.
Affected components: T4; dependency state for T5–T7.
Supersedes: None.
Reason: Existing Git and Vela semantics suffice; the next uncertainty is end-to-end behavior in a real verifier-rich source workflow.
```

## VC1-D009 — Qualify T4 through pending-Proposal replacement

```text
Decision ID: VC1-D009
Date: 2026-08-26
Question: Is inability to apply accepted-Claim correction relations to a failed pending candidate a Core defect that terminates T4?
Proposal: Preserve the refusal, distinguish pre-decision replacement from accepted-Standing revision, and test the documented authenticated withdrawal plus fresh Submission path in a separately frozen continuation.
Evidence: T4 v1 commit d4c88ceb; Vela CLI documentation for review withdraw; T4 v1.1 freeze 7161559d and terminal commit 05b6e36; supervisor in-place and fresh-clone replay.
Alternatives: Accept a known-failing Claim to make it targetable; add a pending-attempt correction object; terminate the vertical; erase the failed attempt.
Decision: The v1 relation refusal is a valid accepted-Standing semantic boundary. The v1 full-vertical stop was an experimental-design failure. T4 v1.1 qualifies using withdrawal -> fresh add_claim -> Verification -> authorized Decision -> Event -> Standing -> replay -> pending continuation. No Core change.
Authority: S0.
Affected components: T4; T6 design; T7 release evidence.
Supersedes: The campaign-level interpretation of T4 v1 as a fatal protocol contradiction; v1 artifacts and report remain immutable.
Reason: A pending Proposal is not Standing. Producer withdrawal preserves its history without scientific authority, while correction/supersession remains reserved for accepted-state revision.
```

T4 qualifies at Level 1 with two explicit limitations: exact zero-byte verifier
streams require a nonempty hash receipt, and the current producer write path
cannot author a canonical `depends` relation for the downstream Claim. Neither
limitation changed Standing or required domain-specific Core semantics.

## VC1-D010 — Authorize T5 independent review only

```text
Decision ID: VC1-D010
Date: 2026-08-26
Question: May the Alzheimer vertical proceed directly from the planning synthesis?
Proposal: Freeze one bounded question, then require an independent field-level primary-source audit before selecting a scientific Repository or executing a Submission.
Evidence: Planning repository b0eb6fc26c8deba2260a1326f5caf6a99153a2b2; missing historical source-owned Alzheimer tree; six-source primary-evidence manifest; unresolved cohort overlap and A−T− evidence boundary.
Alternatives: Import historical summaries as evidence; treat the planning synthesis as Verification; begin accepted Standing immediately.
Decision: Authorize independent review task 01a03d6a-efbf-7e92-9c73-c7328f9ff7e8 only. No T5 scientific lifecycle or Decision is authorized until S0 audits that review.
Authority: S0.
Affected components: T5.
Supersedes: None.
Reason: The selected question is real and bounded, but the exact extraction and strongest proportionate Claim require independent primary-source verification.
```

## VC1-D011 — Reject T5 v1 extraction and authorize correction only

```text
Decision ID: VC1-D011
Date: 2026-08-26
Question: Does the independently reviewed T5 v1 planning packet support scientific lifecycle execution?
Proposal: Accept the review verdict, preserve v1 unchanged, and permit one strict-descendant correction before any lifecycle execution.
Evidence: Planning commit b0eb6fc26c8deba2260a1326f5caf6a99153a2b2; independent-review commit 5eb13c9bec3ecef051682b664ec6e4fa35f63491; supervisor clean-clone, hash, JSON, and Git-integrity audit.
Alternatives: Execute the overstated Claim; treat the review as a negative biological result; discard the source corpus; weaken the independent-review gate.
Decision: REJECT_EXTRACTION. Keep the T5 scientific lifecycle BLOCKED. Preserve v1 and authorize only a v1.1 correction that uses separate Aβ1-42-adjusted and pTau-adjusted model language, avoids formal APOE-effect-modification claims, corrects S1/S2 representation labels, preserves the Preis inconsistency, and binds source-level missingness and attrition. Require independent re-review before execution.
Authority: S0.
Affected components: T5; T7 release evidence.
Supersedes: No prior artifact; narrows the execution authority granted by VC1-D010 after its required review.
Reason: Source identity and retrieval integrity passed, but model role, group comparison, and representation-version fields were materially misstated.
```

The maximum permitted T5 Claim boundary during correction is the reviewer’s
bounded literature statement. It is not a causal, diagnostic, prognostic,
therapeutic, or medical conclusion and is not yet authorized Standing.

## VC1-D012 — Select T6 candidate C and authorize Stage 0 only

```text
Decision ID: VC1-D012
Date: 2026-08-26
Question: Does any pre-outcome T6 continuation task have enough plausible dynamic range to justify evaluator-authorability qualification?
Proposal: Preserve the trivial-task kills, select only finite uniformity aggregate candidate C, and authorize a separate hidden Stage 0 gate.
Evidence: T6 design commits eccd0c8a94ab5999e6f79bf591969f1937d971d5 and 32fbe541da65b3ffa6e73d4d77d866b34aa75eca; internal/external validator PASS; three equivalence/tamper tests PASS; supervisor fresh-clone, Git, source-hash, target-absence, and no-Lean-file audit PASS.
Alternatives: Run the trivial modulus-two task; select A or B; begin three participant arms immediately; replace the task after observing an outcome; close T6 without testing the only survivor.
Decision: Kill the modulus-two, A, and B tasks before outcome. Select C only. Authorize Stage 0 apparatus and hidden evaluator-authorability work. Keep Stage 1 R/E/V cells BLOCKED pending a committed receipt and new S0 audit.
Authority: S0.
Affected components: T6; T7 release evidence.
Supersedes: The specific pending modulus-two continuation from T4 is preserved but not used as an experimental target.
Reason: C introduces finite indexed limit aggregation and Lean elaboration work without turning T6 into theorem discovery, while the other tasks lack measurable continuation cost.
```

Stage 0 must retain the reference proof outside every participant, package,
runner-log, and source root. Failure authoring the exact statement kills the
task; it does not authorize a substitute or a changed statement.

## VC1-D013 — Accept T5 v1.1 extraction and authorize one lifecycle

```text
Decision ID: VC1-D013
Date: 2026-08-26
Question: Does the corrected T5 extraction qualify for one source-owned governed-science lifecycle?
Proposal: Accept PASS_EXTRACTION, freeze the exact literature-report Claim, and run one bounded initial-Standing -> reviewed Submission -> authorized Decision -> updated Standing -> replay -> clean continuation lifecycle.
Evidence: Planning commit/tree 337f8bd836b06474fbbf00be6275b673a950ff7a / f8eb8b1c46ee07f6048098f43019e65bee64d9fb; independent re-review commit/tree 427e234c77ebea6022ca835c0ee236e2c1ab0110 / 38cfa8925c123f27ad002cd23d4e483584d46060; all twelve source representations matched; supervisor JSON, hash, ancestry, Git-integrity, and fresh-clone audits PASS.
Alternatives: Keep planning indefinitely; broaden the Claim; add sources; treat reviewer PASS as Standing; bypass an authorized Decision.
Decision: Authorize VC1-E004 in a new source-owned repository. The maximum positive Claim is the exact reviewer boundary. Bind Preis QAlb n=33 and Edwards source-specific no-multiplicity-correction as mandatory qualifications. Require independent Verification, explicit authority, append-only Events, exact replay, and a clean downstream continuation. No Core change is authorized.
Authority: S0.
Affected components: T5; T6 real-science dependency; T7 release evidence.
Supersedes: VC1-D011's correction-only block after the required re-review; the rejected v1 packet and review remain immutable evidence.
Reason: The corrected extraction is source-stable and proportionate, so the remaining uncertainty is whether current general protocol semantics can govern the real-science transition and preserve enough state for continuation.
```

`PASS_EXTRACTION` is evidence for authorizing an experiment, not a Vela
Verification or scientific Decision. The lifecycle must still earn Standing.

## VC1-D014 — Close malformed T6 v1 and authorize one syntax-only successor

```text
Decision ID: VC1-D014
Date: 2026-08-26
Question: Does the T6 v1 Stage-0 failure constitute a handoff result, and may any corrected apparatus proceed?
Proposal: Close the exact malformed v1 target with no scientific conclusion, preserve every receipt, and permit one strict-descendant v1.1 that repairs only the bounded-sum binder and makes the R Git-metadata package rule internally exact.
Evidence: Hidden-evaluator commit/tree 57e7ea008c57df5343f10e0d75ab141046d7fad8 / cb371cb58b71a8aa264781e9ab93e50ee595fcaa; authorability receipt SHA-256 54f90a2dc24d0cdd0ea8adc654a70ab8f2272cb6132f4e0b7ce200cda53d1c7c; apparatus commit/tree 461c79df4be504b4d0071eb1dd1e90749a2b0f09 / b195cf458e6d827a71bd236867dffbfa0c935b5b; receipt root sha256:50c80085fdf4ba3c4902e1b16fea3e383dc5062985e608f7ebeaa183deb2ff8c; supervisor exact-byte Lean reproduction and in-place/fresh-clone 11/11 test PASS.
Alternatives: Interpret the parse error as negative handoff evidence; mutate v1 in place; substitute a new theorem; waive exact tokenizer identity; abandon the required T6 evidence after a pre-participant typo.
Decision: T6-C1 v1 is closed as EXPERIMENTAL_APPARATUS_FAILURE with scientific denominator zero. Authorize one v1.1 design correction only. The mathematical proposition, candidate-C task class, source bindings, R/E/V semantics, order, budgets, metrics, thresholds, reference-proof isolation, and no-retry rules must remain identical. The exact tokenizer identity and <=5% gate remain mandatory. Stage 1 remains BLOCKED.
Authority: S0.
Affected components: T6; T7 release evidence.
Supersedes: VC1-D012 only for a new, explicitly versioned apparatus artifact; the v1 target and Stage-0 failure remain immutable and terminal.
Reason: The gate failed before any participant or scientific outcome because of a mechanically identifiable parser spelling, not because the proposition was unprovable or the handoff arms lacked value. A transparent versioned apparatus repair preserves falsifiability without laundering or retrying an outcome.
```

This decision does not authorize a changed theorem, an easier target, a model
call, or any R/E/V scientific cell. If v1.1 cannot pass all Stage-0 gates
without another scientific or control change, T6 candidate C closes.

## VC1-D015 — Qualify T5 lifecycle and authorize the blinded successor

```text
Decision ID: VC1-D015
Date: 2026-08-26
Question: Did VC1-E004 complete a valid governed real-science transition, and may the frozen clean-successor test run?
Proposal: Accept the source-owned lifecycle after independent replay/custody audit, then launch exactly one blind successor against a fresh clone and the already-frozen participant package.
Evidence: Lifecycle commit/tree 363d1210e33f951739b6281097054f179ee04123 / ba7c3381899c76ae972a71ed7355c3e1fcfc087c; public receipt SHA-256 1bc4b26523b28b4437ae2b0e47e4cbc79aae3bcf3f9719d8afc9c493aa30a1b9; package receipt SHA-256 acc892381d14dd529aae248dea9d7c7a18a642b3ae3cac7dcdd49e42dc16fd28; terminal Repository root sha256:785fa897ac8ffa9e8dd92756090923ee9e8ce3ec593ed07bb73baa63aa58a79a; supervisor in-place and fresh-clone validation/replay/Git audits PASS; separate evaluator key commit/tree affe34ba5933865f1a8705e40285131a29ec7b33 / 687b1a1c56f663fe7f721a167d18e10b90edace4.
Alternatives: Treat reviewer PASS as the lifecycle result; skip blinded continuation; expose the answer key; authorize the successor to change Standing; generalize the bounded literature result.
Decision: VC1-E004 lifecycle PASS. Authorize one fresh clean-successor task under the frozen 15-minute, 15,000-observable-token, and 40-tool-call caps. It must answer 10/10 questions, name one bounded useful action, create exactly one pending Proposal, and leave accepted Standing and existing Events unchanged. No Verification, Decision, rejection, withdrawal, network, new source, outreach, publication, push, or scientific generation is authorized.
Authority: S0.
Affected components: T5; T6 real-science dependency; T7 release evidence.
Supersedes: VC1-D013's post-lifecycle launch block after all required lifecycle evidence passed.
Reason: General Vela semantics governed the exact literature transition and replayed it without a domain special case; the remaining T5 uncertainty is whether a blind successor can correctly comprehend and continue from committed Standing.
```

Lifecycle PASS is Level-1 internal protocol evidence. It is not a biological,
medical, adoption, productivity, or cumulative-intelligence result. The clean
successor must earn its own separately scored outcome.

## VC1-D016 — Accept T6 v1.1 design correction and reauthorize Stage 0

```text
Decision ID: VC1-D016
Date: 2026-08-26
Question: Does the additive candidate-C v1.1 design preserve the frozen science and controls closely enough to rerun only Stage 0?
Proposal: Accept the one-span Lean syntax correction and exact common Git-checkout contract, then rerun independent hidden authorability and apparatus qualification with no participant context.
Evidence: Design commit/tree 169fd94724048c7d30d780ccceb3f3c7b38b18eb / 6a6da4dac8a0085bd5bbf5fbbd9c9d0d98cd7120; direct parent 32fbe541da65b3ffa6e73d4d77d866b34aa75eca; corrected target SHA-256 aa18f3bb86b194c4ec1b481a803eb11b183f3c0f3ccefbd1e971695a19b225fb; diff receipt SHA-256 5e828cba203b48b76d44c750a14ba71ff9809d331f9eb027c722fc5d699d0836; unchanged-control receipt SHA-256 073c10e1559649db5bb007379a2f68d323b97f7da6f8f42f6327069f05b92100; elaboration receipt SHA-256 50f658a6904b0b6e46c691aa76a0ce0f03a92de72429f697dfc248c199e14954; supervisor internal/external, ancestry, exact-v1-tree, test, Git, and fresh-clone audits PASS.
Alternatives: Mutate v1; reuse the old failed apparatus receipts; change the theorem or budgets; waive tokenizer identity; begin participant cells after proposition-only elaboration.
Decision: Accept the v1.1 design correction. Authorize separate hidden evaluator-authorability and apparatus Stage-0 tasks only. They must rebuild versioned packages and receipts from v1.1, keep the reference proof sealed, start no participant/model/scientific cell, preserve v1 repositories unchanged, and retain exact runtime tokenizer identity plus <=5% token equivalence as fail-closed gates. Stage 1 remains BLOCKED pending both committed PASS receipts and a new S0 audit.
Authority: S0.
Affected components: T6; T7 release evidence.
Supersedes: VC1-D014's design-correction-only block after the exact correction passed audit; v1 remains terminal and immutable.
Reason: The malformed artifact was corrected without changing the proposition or experiment. Independent Stage-0 requalification is now the next falsifiable gate.
```

Target proposition elaboration is not hidden proof authorability and is not a
scientific result. Any additional correction, proxy tokenizer, or participant
start before a later Decision closes candidate C.
