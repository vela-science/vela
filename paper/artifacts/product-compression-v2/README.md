# Product-compression v2 — native Harbor result

This directory retains only the compact, reviewable result from the first
complete native Harbor comparison. Its generated Harbor cache was removed from
the repository workspace; the compact result retains the exact job root. Raw
tasks, container logs, trajectories, and viewer state were execution cache,
not Vela source or scientific Standing.

## Result

- frozen plan: `sha256:3ec7c05814add867dc096887f6d8af5deca16fe70eb883e86c76747ba10f8598`
- raw Harbor job: `sha256:a14978b2508a10ea70fc48fcde3d8ad7ff3e3264e407343bac28b5b1872bc9f9`
- compact result: `sha256:d04426beaf8d6f7eb96ef48a67a05d8356bc3f52d0fb986e37c96355fe7393d0`
- execution: four counterbalanced Codex sessions, zero execution exceptions,
  zero semantic interventions, zero authority availability, and zero
  scientific-state change
- registered outcome: **failed; no product-lift credit**

The Vela-guided arm was faster in both pairs. Its median elapsed time was
191,459.5 ms versus 272,681 ms for Git/files, a 29.78 percent improvement. It
also used fewer median tool calls and observed tokens. Those directional
signals do not satisfy the registered gate: only one answer matched the frozen
key exactly, every session exceeded the 24,000-token limit, and three exceeded
the 12-tool limit.

## What the failure taught us

The run found two benchmark defects that remain visible rather than being
silently repaired after output:

1. The offline verifier captured `git status` as bytes and compared it with a
   string. It therefore marked every clean checkout dirty. Final trajectories
   independently show empty status output, and answers were written outside
   the checkout.
2. `accepted_before` and `accepted_if_*` did not say whether they represented
   global Standing or only the target-specific conditional delta. The frozen
   key meant the latter; two agents reasonably returned the former. A later
   contract must name the target-scoped delta explicitly.

Harbor's low-entropy secret redaction also replaced the value `true` with an
unquoted marker inside per-trial JSON and ATIF observations. The top-level job
summary and recorded raw-file roots remain preserved, but the corrupt raw
per-trial reward is not treated as valid.

The product conclusion is deliberately narrow: Vela's read path is
directionally more efficient, but the current Decision-packet interface has
not earned a product-lift claim. The next study requires an actual product
change that presents one explicit target-scoped Standing delta; it receives a
new plan root and corrected verifier. No new runner, adapter, service, or
framework is earned by this result.
