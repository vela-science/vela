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

## VRC1-S008 — Gate 1 accepted; external-legibility phase opened

Recorded: 2026-08-26, America/Toronto.

Fresh R1 independently returned `PASS WITH DOC FIXES` after exercising every
named governed read under missing, malformed, mismatched, correct, and
hostile-`HOME` trust conditions; unpinned routine evidence writes and pinned
Decision admission; the portable verifier; the complete locked Core union;
and the release-candidate lint gate. Its evidence is frozen in
`R1_REQUALIFICATION.md` and audit-only integration test
`r1_requalification.rs`.

Fresh R2 independently returned
`PASS WITH DOCUMENTED PLATFORM LIMITATIONS` from pristine Linux x86-64 guests.
It built and installed the exact repaired candidate, exercised the governed
operator loop, read public Math only after installing its independently
published pin, reproduced the neutral replay fixture, and observed fail-closed
behavior for missing/corrupt Artifacts and missing/malformed/mismatched pins.
Disposable macOS remains untested. Its evidence is frozen in
`R2_REQUALIFICATION.md`.

S0 integrated both reports and independently reran the R1 trust matrix and
neutral replay fixture successfully. VRC1-F004 is resolved on repaired product
commit `ad2a4516078525025d05bd461b550ed5b8e35971`. G1 is
`PASS WITH DOC FIXES`; G2 is `PASS WITH DOCUMENTED PLATFORM LIMITATIONS`.
R3 and R4 are now authorized from the reviewed supervisor branch. R5-R7 remain
blocked.

## VRC1-S009 — external legibility qualified; product/release audits opened

Recorded: 2026-08-26, America/Toronto.

R3 returned `PASS WITH DOCUMENTED LIMITATIONS`. It corrected the Proposal-root
catalogue, release-checklist commitments, public semantic scenario index,
installation and Git prerequisites, Quickstart verification requirement, and
stale trust-pin examples. S0 integrated the change and independently reran the
documentation contract and portable verifier.

R4 returned `PASS WITH DOCUMENTED LIMITATIONS`. It supplied two independently
replayed public examples with no Core or schema fork: a finite formal verifier
lifecycle preserving failure, rejection, correction, acceptance, and replay;
and a heterogeneous computational-evidence lifecycle requiring two scoped
checks before Decision. The exact example Repository roots are recorded in
`R4_EXTERNAL_FIXTURES.md`.

After additive R3/R4 integration, S0 regenerated the one authoritative
informative manifest and passed the portable verifier, 14 documentation
contracts, both focused R4 integration tests, and both public check scripts.
The merged Protocol 1 manifest contains 77 normative and 67 informative files
with root
`sha256:553c2bf5b495506e5297027c47abd68e058f1a34136900fc4e4606c81d311a17`.

G3 and G4 are accepted with their explicit unpublished-candidate, platform,
and pending-blind-test limitations. R5 product legibility and R6 packaging /
release integrity are now authorized. R7 remains blocked until S0 accepts both.

## VRC1-S010 — R5/R6 holds accepted; bounded repairs opened

Recorded: 2026-08-26, America/Toronto.

R5 returned `HOLD — PRODUCT SEMANTICS`. Core and Workbench preserve the
qualified distinctions, but the current Problems/Observatory projection calls
a legacy Vela 0.977.3 result `strict pass` even though that generator predates
the mandatory independent trust selection. Its Repository overview also labels
an actor-neutral count containing agent Decisions as `Human authority`.

R6 returned `HOLD — RELEASE INTEGRITY`. Artifact-to-source traceability,
reproducibility machinery, installer verification, dependency policy, and the
existing public ancestor's signatures/provenance pass. The distributable
archives themselves contain only the executable: they omit Vela project
license texts and deterministic third-party notice material, while the SPDX
documents leave all package license/copyright fields `NOASSERTION`. R6
recommends a later `PATCH BUMP` to 0.977.5; no version change is authorized now.

S0 accepts both holds without broadening scope. One isolated Vela Web repair
may make authority labels actor-neutral and prevent legacy/unpinned projections
from presenting current Protocol 1 strict integrity. One isolated Vela release
repair may stage deterministic project licenses and third-party notices and
make smoke/reproducibility tests refuse omissions. R7 remains blocked pending
fresh independent R5/R6 requalification.

## VRC1-S011 — product and packaging source gates accepted; blind test opened

Recorded: 2026-08-26, America/Toronto.

R5 independently returned `PASS WITH DEPLOYMENT/REPROJECTION ACTIONS` on Vela
Web commit/tree `fd2bb321e2331b546ad5f94705707af9d087ddaa` /
`1f33b0f65b7fa17be2cadcfc2c3a942ff4acffb4`. The repaired source rejects
self-asserted legacy-generator provenance and unknown, missing, duplicate, or
mismatched Repository authority roots. It also labels the actor-neutral count
`Authorized Decisions`. The currently deployed legacy projection remains
unqualified until a separately authorized release, reprojection, live-reader
audit, and deployment.

R6 independently returned `PASS WITH DOCUMENTED RELEASE-TIME ACTIONS` on Core
commit/tree `bd18d1a128eecb95dfd3bfd6cfe198f109576c78` /
`7187138a3025e391f8cd467abc634b7b5bb73ff4`. Fresh-cache macOS and Linux musl
release builds passed, with exact selected-graph, notice, SPDX, relationship,
archive-negative, installer, conformance, Core, lint, and policy checks. The
audited artifacts remain local, unsigned, unattested, and unpublished.

G5 source qualification passes with recorded release-time actions. R7 is now
authorized against an exact clean release-facing snapshot. Version remains
`0.977.4` during qualification; the later release recommendation is a bounded
patch bump to `0.977.5`. No bump, tag, push, signature, publication,
deployment, or release is authorized.

## VRC1-S012 — blind user passed; release-ready verdict recorded

Recorded: 2026-08-26, America/Toronto.

R7 returned `PASS WITH FIRST-USER LIMITATIONS` on exact candidate commit/tree
`41ec11750daf8268eba61f9307fe0bcbbd6ca044` /
`233d4713bcc6112aa3a4b9fdf64cddd0a69d6e02`. Without campaign records or
coaching, the participant built Vela, preserved and rejected a failed
Verification, verified a correction, observed that passing Verification still
left Standing empty, made the separate authorized Decision, inspected retained
history, replayed the same governed state in a fresh clone, and diagnosed a
missing Artifact as fail-closed.

The participant correctly stated all six required semantics and identified a
concrete value over ad hoc Git plus JSON plus logs. Its scope-selection restart,
corrupt-fixture pin-order friction, account-scoped trust-anchor cleanup, and
warm Cargo cache are preserved as first-user limitations.

VELA-RC-1 therefore records `RELEASE READY WITH EXPLICIT LIMITATIONS` and
returns `READY FOR USER AUTHORIZATION WITH LIMITATIONS`. This is a
qualification verdict, not a release action. A later authorized release must
prepare and requalify the bounded `0.977.5` tree and must separately authorize
Vela Web reprojection and deployment. Foundational top-down search remains
`CLOSED`.
