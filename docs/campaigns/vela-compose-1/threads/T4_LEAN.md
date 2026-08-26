# T4 — Lean verifier-rich vertical worker contract

Status: blocked until stable general APIs and S0 launch.

This is a protocol stress test in the source-owning Lean repository, not a
theorem-proving campaign and not Core domain logic. Use an existing real Lean
workflow and freeze repository commit, Lean/mathlib versions, theorem, inputs,
and authority before execution.

Required lifecycle: problem -> Submission -> rejected Lean Verification with
preserved evidence -> corrected Submission -> successful Verification ->
authorized Decision -> Event -> Standing -> clean replay -> downstream
continuation. Where cheap, exercise conflicting checks, supersession, invalid
environment, and branch comparison.

Do not special-case Lean into Core. Classify every defect using the campaign
failure taxonomy and report exact event sequence, receipts, replay, defects,
artifacts, and commit. Do not merge.

