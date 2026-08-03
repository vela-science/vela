# Erdős 730 external-proof-boundary campaign

## Objective

Determine whether the pinned complete Lean proof establishes the exact retained
Erdős 730 statement, preserve the Lean 4.29.1/4.27.0 boundary, and prepare a
consequence-complete packet for an explicit human Decision or deferral.

## Frozen entry state

- Frontier commit: `ea44055f33ec04509385454228fd6cba8fcfe562`
- Target: `erdos:730:external-proof-boundary`
- Packet root:
  `sha256:36dd946797305295d127d5c6fed23ffccd76609a8705f0155c9cf2f7f1c6e370`
- Output: `artifacts/fidelity/erdos-730-proof-boundary.v1.json`
- Verifier: `execution/erdos-730-proof-boundary/verify.py`
- External terminal theorem:
  `Erdos730.FullDensityTheorem.pairSet_infinite`
- External toolchain: Lean 4.29.1
- Formal Conjectures toolchain: Lean 4.27.0

## Current result — 2026-08-03

The producer-side boundary report now exists at the frozen output path in the
Erdős Frontier. The source-local verifier recomputed both Git trees and source
roots, proved terminal-solve ancestry and unchanged terminal bytes, scanned all
74 retained Erdős 730 modules for proof escapes, and then compiled
the terminal theorem and audit from a clean detached checkout under Lean
4.29.1. It returned `outcome: pass`, `external_native_lean_passed: true`, and
report root
`sha256:42db39dd2b51e7821e02fc1acbb3e43cde83f269a8cb491f2925ad3aa233d106`.

The report concludes `equivalent`: the external `PairSet` repeats the exact
Formal Conjectures predicate, while the constructed consecutive-pair family is
a stronger witness shape. It preserves the Lean 4.29.1/4.27.0 boundary. The
campaign record explicitly treats this as same-operator, same-machine producer
evidence with shared source, Lean kernel, and Mathlib dependencies.

The producer result itself is non-authoritative. It is retained by Submission
`vsb_46bd0d7cef0d2fa6` and Proposal `vpr_c9554694d438c594`. A fresh ephemeral,
source-first reviewer independently
recomputed the exact sources, compiled the terminal theorem and audit, checked
all six semantic dimensions, and retained requirement-satisfying scoped
Verification `vvr_3b6d523c55a24dc9`. The review shares the same human operator,
machine, Codex model family, source repositories, Lean kernel, and Mathlib; no
external-participant or organizational independence is claimed.

The Decision Inbox was protocol-ready with no blocker at exact entry root
`sha256:cdbf8a2919a1c6ac137ab21468e74d6556bdebd4c250f7aee6adf45a76c3400d`.
Before Decision, the repository root was
`sha256:db438141c7780f1122ee11daf7a57390a275dfc03744131ad991e9a65bbd39b9`.
The attributed human Decision accepted only Claim
`vcl_8ef85fca44b8d9105e8c28b9ba702accd9365c4ff23d87466bf2b64853921345`
through event `vev_0ab843df6ad373ec`, yielding the predicted root
`sha256:821cf0d94778f647305107943572f4916a6cf63fe5ea12506a471fabc07b7474`;
strict replay passes at Frontier commit
`9ecb63bc97ccb8b403b4088e15c54499ab4e95f6`. No unrelated Standing changed.

The completed Target is no longer offered by `vela next`. Its exact
post-Decision handoff is retained at
`execution/erdos-730-proof-boundary/post-decision-handoff.v1.json`, root
`sha256:e1236ab59f36ab655dbbfdc2bc6d147554afd36040f18a7e14d7762cad5916d7`.
The primary next product obligation is the second non-authoritative Result
Dossier case. External mathematical review or a native Lean 4.27.0 bridge
remains separate future scientific work.

## Work packages

### 1. Source and lineage custody

- Recompute both Git commits, trees, relevant blobs, and terminal-solve
  ancestry.
- Confirm the terminal theorem bytes are unchanged in the pinned snapshot.
- Bind all 74 Erdős 730 modules without copying them into the Frontier.

### 2. Mechanical replay

- Compile the terminal theorem and audit under the pinned 4.29.1 environment.
- Reject `sorry`, `admit`, new `axiom`, `opaque`, and `unsafe` escape paths.
- Record exact axioms, toolchain, Mathlib, heartbeat, platform, and network
  contract.
- Confirm exact Formal Conjectures statement bytes and 4.27.0 environment.

### 3. Independent semantic matrix

Review the two sources across:

1. quantified domain and consecutive-pair order;
2. central-binomial coefficient definition;
3. prime-factor support and equality predicate;
4. conclusion strength and implication direction;
5. hidden assumptions, axioms, and imported lemmas; and
6. cross-toolchain import or bridge requirements.

The semantic reviewer must not copy the producer's conclusions. Shared source,
operator, machine, kernel, and library dependencies are disclosed.

### 4. Verification and packet

- Produce exactly `equivalent`, `not_equivalent`, or `indeterminate`.
- Import a scoped Verification over the exact report and source roots.
- Build a Decision packet showing all possible Standing consequences,
  surviving external statuses, discrepancies, and next obligations.
- Ask the authorized human to accept, reject, or defer; never infer a choice.

### 5. Replay and inheritance

After a Decision or documented deferral:

- replay the exact repository state;
- confirm no unrelated Standing changed;
- materialize the next obligation: external-boundary maintenance, a 4.27.0
  bridge/port, discrepancy repair, or external mathematical review; and
- prepare the second non-authoritative Result Dossier case.

## Success gate

- exact source and terminal lineage reproduced;
- native theorem replay passes;
- every equivalence dimension receives evidence and a conclusion;
- toolchain boundary remains explicit;
- scoped Verification passes without implying acceptance;
- consequence-complete human packet exists; and
- replay identifies the exact next obligation.

## Nonclaims

No Vela-caused discovery, novelty determination, global solution status,
community acceptance, external independence, or automatic Standing follows.

## Stop conditions

Stop and report `indeterminate` or `not_equivalent` if source bytes cannot be
reproduced, the theorem fails in its pinned environment, an implication gap is
found, or the toolchain bridge requires unreviewed semantic change. Do not
silently port, weaken, or restate the theorem to obtain a positive result.
