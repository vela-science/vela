# VELA-RC-1 append-only state

Do not rewrite earlier entries. Corrections are later entries.

## VRC1-S001 — campaign opened

Recorded: 2026-08-26, America/Toronto.

```text
Campaign: VELA-RC-1
Phase: PHASE 0 — BASELINE
Status: ACTIVE
Candidate: 421cdc0dc9e9aee57b604e57a8bf5401ab957645
Candidate tree: 24d1f8a314db2dc728ab6a01f0c2ada29bbba0e0
Vela: 0.977.4
Protocol: 1 release candidate
Release: NOT AUTHORIZED
Foundational top-down search: CLOSED
```

R1 semantic audit and R2 clean-install qualification are the only lanes
authorized to start after this control root is committed. R3-R7 remain gated.

## VRC1-S002 — baseline reproduction started

Recorded: 2026-08-26, America/Toronto.

The baseline checkout was clean. The independent portable Protocol 1 verifier
reproduced 77 normative and 39 informative files and root
`sha256:e7a6d288918692d6a6186cc3e612871f167ba954c4cc31de28cce182a66a0afd`.
The exact source-owned T4 and T5 Repositories replayed successfully at their
recorded commits, trees, and roots. The full Core union and release-candidate
clippy gate remain pending.

## VRC1-S003 — Phase-0 inherited qualification reproduced

Recorded: 2026-08-26, America/Toronto.

From supervisor control commit
`6d680eebb4a17813e72b55685aa2eec6b34e5fae`, the complete locked Core union
passed. It covered the portable Protocol 1 verifier, independent current-object
implementations and readers, wire schemas, correction impact, authority-chain
refusals, reference flows, deterministic release-reproducibility fixtures,
Decision Inbox v3, the workspace all-target suite, the workspace
`vela-cli/test-support` suite, and documentation tests. External Lean was not
selected by the Core union, as documented.

`cargo clippy --locked --workspace --all-targets -- -D warnings` also passed.
The supervisor worktree remained clean before this state update. Phase 0 finds
no inherited semantic or replay regression. R1 and R2 are active; their gates
remain unadjudicated.

## VRC1-S004 — R1 semantic gate accepted as HOLD

Recorded: 2026-08-26, America/Toronto.

S0 independently checked R1's cited normative text, production loader, status
projection, informative profile caveat, and passing genesis test. The finding
reproduces: Protocol 1 requires strict replay to verify an independently pinned
sequence-one authority root, while `vela replay` and read/status paths validate
the repository-retained authority history without loading the local pin and
can report `strict: pass` after that pin is removed. Decision writes remain
fail-closed, but this does not satisfy the normative read/replay contract.

R1 worker commit `6501f687` was integrated as supervisor commit
`d2ac9d88d232eb35fc7f3f575ebd9fd438b7daee`. Gate G1 is
`HOLD — SEMANTIC BLOCKER`. R2 may finish its already authorized independent
clean-install evidence, but R3-R7 remain blocked. No release-facing redesign,
Protocol weakening, version change, tag, push, publication, or release is
authorized.
