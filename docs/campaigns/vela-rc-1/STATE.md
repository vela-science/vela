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

## VRC1-S005 — R2 clean-install gate accepted with limitations

Recorded: 2026-08-26, America/Toronto.

R2 independently exercised the signed `v0.977.4` install and public Math read
path in a clean Debian x86-64 guest, then built exact RC-1 control commit
`6d680eeb` in a separate clean Rust 1.97.1 guest and completed
`init -> submit -> verify -> decide -> status -> replay`. A second pristine
consumer reproduced the frozen neutral arithmetic history and exact Repository
root; a sibling history with a missing required Artifact failed closed with no
partial Standing. The focused checked-in regression passed independently under
S0.

R2 worker commit `e6e7bbcc1c48a1abea3a4ea427e7fac4e894d433` was integrated
as supervisor commit `c2808440`. Gate G2 is
`PASS WITH DOCUMENTED PLATFORM LIMITATIONS`: Linux x86-64 was tested under
emulation; disposable macOS was not. R2 independently reproduced the R1
unpinned-read blocker, so the overall candidate remains on HOLD and R3-R7 stay
blocked.

## VRC1-S006 — bounded authority-read repair opened

Recorded: 2026-08-26, America/Toronto.

VRC1-D007 opens one isolated repair lane from supervisor commit
`4d6109c98ecb816434109b8d6884bb3bec0eec7a`. Its only product objective is to
make the existing normative independent sequence-one selection real on shipped
governed-state reads while preserving routine unprivileged evidence writes.
R1/R2 requalification and every downstream gate remain closed pending a
committed repair and independent supervisor audit.

## VRC1-S007 — authority-read repair integrated and locally qualified

Recorded: 2026-08-26, America/Toronto.

The bounded repair was integrated at supervisor commit
`ad2a4516078525025d05bd461b550ed5b8e35971`, tree
`e08112922efbe59ef3b042d0a8f6b0f9557761ea`. It binds governed-state reads to
the existing operating-system-account trust anchor, fails closed when that
anchor is missing, malformed, or mismatched, and preserves ordinary
unprivileged Submission, Verification, and withdrawal writes. Public read
instructions and the neutral replay fixture now install the independently
published sequence-one root explicitly.

S0 independently passed formatting, diff hygiene, the direct trust-boundary
matrix, the focused public fixture and authority suites, the portable
conformance verifier, release-candidate clippy, and the complete locked Core
union. The repaired Protocol 1 conformance root is
`sha256:6a9d475c11db78faeb239a2f6c55b369b8b9a3f79c26c92cb59b7ae5eb2eb5d4`.

This is implementation and supervisor evidence, not independent gate
acceptance. G1 and G2 are `REQUALIFICATION PENDING`; fresh R1 and R2 auditors
must qualify this exact commit/tree before R3 or R4 may start.
