# Legacy Policy Retirement Implementation Plan

> **For Codex:** Work only in this checkout. This plan prepares a pending governance proposal; the existing human `vela sign` ceremony remains the only keyed authority path. Never read a human key or accept, sign, or finalize a proposal while implementing or testing it.

**Goal:** Add a closed, auditable recovery lane for retiring a prelaunch active-policy byte pair that current policy parsing cannot validate, without weakening normal policy validation or authority custody.

**Architecture:** A keyless `vela policy retire-legacy` command inspects bounded raw policy and signature envelopes, proves the pair is unused and predates policy-head governance, and records a content-bound pending proposal. The existing decision-plan and `vela sign` path rechecks those facts under lock and, on an authorized human acceptance, atomically appends the ordinary signed review event while deleting only the fixed active pair and exact byte-identical snapshots. Separately, strict scientific signals recognize only the typed, non-biomedical Erdős catalogue shape and restrict the biomedical `translation` heuristic to positive biomedical context.

**Tech Stack:** Rust workspace, Clap CLI, serde/serde_json, SHA-256 content roots, Vela proposal/review/decision-plan protocol, focused Cargo tests.

---

1. Define the closed retirement payload and bounded raw legacy-pair observation in `vela-protocol`; validate proposal shape without treating legacy bytes as current policy authority.
2. Add a live safety audit that rejects current policy heads, any policy admission history, mismatched envelopes, mutable/symlinked/oversized inputs, non-identical snapshots, replay failure, or byte-root drift.
3. Add `vela policy retire-legacy` as a prepare-only command with required reason and actor, then integrate the accepted proposal with the existing review material and decision-plan ceremony.
4. Add focused regressions for the exact Erdős database-import/theoretical fixtures and mathematical `translation property` false positive.
5. Document the recovery boundary and run only focused protocol/CLI/signal tests; do not invoke external verifiers or broad release suites.
