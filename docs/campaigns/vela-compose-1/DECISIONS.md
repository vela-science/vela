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
