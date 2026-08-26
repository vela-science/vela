# Frozen experiment registry

No outcome-bearing empirical experiment is authorized in Phase 0 or the initial
T1/T2 engineering phase. Baseline conformance checks are engineering evidence,
not scientific cells.

Before any T4, T5, or T6 run, append a frozen entry containing exactly:

```text
Experiment ID:
Question:
Hypothesis:
Treatment:
Controls:
Task/data:
Model:
Tools:
Information available:
Resource budget:
Primary outcome:
Secondary outcomes:
Inclusion criteria:
Exclusion criteria:
Stopping rule:
Interpretation table:
Artifacts to preserve:
Frozen commit and hashes:
Execution authority:
```

Success criteria never change after outcome inspection. Invalid and excluded
runs remain preserved in attempted denominators.

## VC1-E001 — T4 Lean verifier-rich vertical v1

This central entry is a post-run mirror. The authoritative preregistration was
committed in the source-owned experiment repository before execution at
`3894a54b8f88f4e07d1520e10ce1f7f07ad44815`; the omission of a contemporaneous
central mirror is retained as a campaign-documentation defect and does not
rewrite that freeze.

```text
Experiment ID: VC1-E001 / T4-v1
Question: Can the current protocol retain a failed real Lean attempt and then relate a corrected Submission to that non-accepted candidate through --supersedes?
Hypothesis: The frozen relation attempt will either admit a non-distorting lifecycle or expose an exact semantic boundary.
Treatment: One natural failed proof, failed Lean Verification, and one corrected --supersedes Submission targeting the failed candidate Claim.
Controls: Frozen source/toolchain/artifact hashes; zero accepted Standing before and after Verification; no acceptance of the known-failing attempt.
Task/data: Erdos154.erdos_154_sumset at lean-proofs commit 06d1322e62aa28b860da1ec66465d913c1902c78.
Model: None; already-published proof fixture, no proof search.
Tools: Vela 0.977.4; Lean 4.29.1; Mathlib 5e932f97dd25535344f80f9dd8da3aab83df0fe6.
Information available: Frozen theorem, natural missing-bridge attempt, corrected published fixture, exact source environment.
Resource budget: One attempted Lean execution; stop before corrected execution if relation is refused.
Primary outcome: Whether the exact non-accepted candidate can be the correction/supersession target without changing predecessor Standing.
Secondary outcomes: Failed-evidence retention, accepted-Standing invariance, replay, exact output custody.
Inclusion criteria: Exact frozen hashes and expected natural Lean failure.
Exclusion criteria: Parser/binding invocations that create no Verification; all preserved.
Stopping rule: Stop on relation refusal, hash mismatch, unexpected Lean result, new axiom, replay mismatch, or required Core change.
Interpretation table: Admit = continue frozen lifecycle; refuse because target is non-accepted = qualified semantic boundary; any distorted acceptance = protocol failure.
Artifacts to preserve: Freeze, attempt bytes, exact streams/status, failed Verification, refusals, Event/Standing audit, replay.
Frozen commit and hashes: 3894a54b8f88f4e07d1520e10ce1f7f07ad44815; target SHA-256 9ac3fc83bbeba2df4739b5f3d69130876d99ea09c47d0c30977339904d74f457.
Execution authority: T4 task 01a03d3f-a006-7c51-85cd-b3392703f581; source-owned repository authority only.
```

Terminal source-owned commit: `d4c88ceb64738f29b804a4dc7e735272bd873dd5`.
Result: the relation was refused because the predecessor was not accepted.
Supervisor adjudication: the semantic boundary is valid, but the full-vertical
stop was an `EXPERIMENTAL_DESIGN_FAILURE` because pending-Proposal replacement
has a separate authenticated withdrawal path.

## VC1-E002 — T4 Lean verifier-rich vertical v1.1

The authoritative v1.1 freeze was committed before continuation execution at
`7161559d` in the same source-owned repository. V1 evidence remained immutable.

```text
Experiment ID: VC1-E002 / T4-v1.1
Question: Can the documented withdrawal -> fresh Submission path complete the real Lean lifecycle while preserving the failed attempt and keeping Verification non-authorizing?
Hypothesis: Existing general APIs suffice without Lean-specific Core changes.
Treatment: Authenticated withdrawal of the failed pending Proposal; fresh corrected add_claim; one corrected Lean execution; passing Verification; one authorized accept Decision; downstream start.
Controls: Immutable v1 commit/root/evidence; exact producer-key continuity; frozen source/toolchain/binary/artifact hashes; before/after Standing audits; clean-clone replay.
Task/data: Same frozen Erdos154.erdos_154_sumset fixture and source environment as VC1-E001.
Model: None; no theorem search or capability evaluation.
Tools: Vela 0.977.4; Lean 4.29.1; Mathlib 5e932f97dd25535344f80f9dd8da3aab83df0fe6.
Information available: V1 retained failure, corrected published proof fixture, documented pending-Proposal withdrawal semantics.
Resource budget: One withdrawal, one corrected Submission, one corrected Lean execution, one passing Verification, one accept Decision, one pending downstream Submission.
Primary outcome: Full Submission -> Verification -> Decision -> Event -> Standing lifecycle with failed history retained and exact replay.
Secondary outcomes: Allowed-axiom audit; producer withdrawal authority; downstream continuation; dependency-write limitation.
Inclusion criteria: Exact v1 preservation and all frozen input hashes.
Exclusion criteria: Any fail-closed no-write command would be preserved; none occurred in v1.1.
Stopping rule: Stop on hash mismatch, unexpected Lean result, disallowed axiom, stale Decision root, replay mismatch, or required Core change.
Interpretation table: Complete with no Core change = Level-1 vertical qualification; Verification changes Standing = fatal invariant failure; replay mismatch = failure; downstream only source-bound = qualify limitation.
Artifacts to preserve: V1.1 freeze, lineage/withdrawal receipts, Lean streams/status/axioms, Verification, Inbox/Decision/Events/Standing, replay, downstream task and receipt.
Frozen commit and hashes: v1.1 freeze 7161559d; v1 predecessor d4c88ceb64738f29b804a4dc7e735272bd873dd5; target SHA-256 9ac3fc83bbeba2df4739b5f3d69130876d99ea09c47d0c30977339904d74f457.
Execution authority: Supervisor RETURN TO WORKER; T4 task 01a03d3f-a006-7c51-85cd-b3392703f581; one source-owned authorized Decision.
```

Terminal source-owned commit/tree: `05b6e36fb46b840eeac533658faf6f71ad99dc06` /
`4b491446071efc4d6cd306397fa33e8b008e2f29`. Terminal Vela root:
`sha256:1f18d90faec38dfb602d1f6bfa51c0f7eb69373698baeb4e8f73cbf5dba5c82c`.
Supervisor replay reproduced all counts and the exact root from a fresh clone.
