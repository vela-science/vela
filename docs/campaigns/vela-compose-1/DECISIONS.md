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
