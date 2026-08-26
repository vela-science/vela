# VELA-RC-1 append-only decisions

Do not rewrite earlier entries. Supersede a decision with a later entry.

## VRC1-D001 — freeze exact inherited candidate

Decision: qualify commit/tree
`421cdc0dc9e9aee57b604e57a8bf5401ab957645` /
`24d1f8a314db2dc728ab6a01f0c2ada29bbba0e0` as the RC-1 baseline. Preserve all
pre-existing local state. No version change is inferred from opening RC-1.

## VRC1-D002 — preserve research closure and release authority boundary

Decision: foundational top-down search remains `CLOSED`. RC-1 cannot reopen a
research lane, cumulative-handoff experiment, or medical study. No tag, push,
publication, version bump, release, deployment, signing, or outreach is
authorized.

## VRC1-D003 — phase-gated workers

Decision: authorize R1 and R2 only after the Phase-0 control commit. R3 and R4
require R1/R2 pass; R5/R6 follow the legibility gate; R7 requires R1-R4 pass.
The supervisor alone integrates reviewed, commit-bound changes.

## VRC1-D004 — initial version disposition

Decision: `KEEP VERSION DURING QUALIFICATION`. Vela remains `0.977.4`, Protocol
1 remains a release candidate, Submission remains v3, and migration remains
none until evidence of an actual semantic, schema, wire, persisted-data, CLI,
or release-facing change supports a later explicit version decision.

## VRC1-D005 — accept R1 HOLD without reinterpretation

Decision: accept R1's `HOLD — SEMANTIC BLOCKER`. The normative independent
trust-anchor requirement and the shipped read/replay behavior disagree. Do not
paper over the mismatch by weakening documentation or renaming the current
read result. Any repair must preserve external authority selection, make the
shipped CLI fail closed on a missing or mismatched anchor, add direct negative
coverage, and rerun R1 before G1 can pass. R3-R7 remain blocked.

## VRC1-D006 — accept R2 with documented platform limitations

Decision: accept R2's clean signed install, exact-candidate source build,
complete governed operator loop, deterministic clean-clone neutral replay, and
missing-Artifact fail-closed fixture. Record Linux x86-64-under-emulation as the
qualified clean platform; macOS disposable qualification remains untested.
This R2 pass does not override VRC1-D005. The neutral fixture is release-facing
evidence, not adoption, utility, or a new protocol root.

## VRC1-D007 — authorize one bounded R1-F001 repair

Decision: authorize one implementation lane to enforce the already normative
independent sequence-one selection on every public governed-state read. It must
use the existing operating-system-account trust store, preserve unprivileged
producer and verifier writes, fail closed on absent or mismatched pins, add
direct shipped-CLI coverage, and update public read instructions. It may not
weaken Protocol 1, auto-trust Repository bytes, introduce a second replay mode,
bump a version, or open R3-R7. Independent R1 and R2 requalification is required
before the semantic gate can change.

## VRC1-D008 — freeze repaired candidate for independent requalification

Decision: accept commit/tree
`ad2a4516078525025d05bd461b550ed5b8e35971` /
`e08112922efbe59ef3b042d0a8f6b0f9557761ea` as the sole repaired candidate for
fresh R1 semantic and R2 clean-install/replay requalification. S0's passing
focused matrix, portable conformance verifier, clippy gate, and complete Core
union establish implementation readiness for audit but do not adjudicate the
release gates. R1 and R2 may inspect and test but may not repair, weaken, or
reinterpret the candidate. R3-R7 remain blocked until both requalifications
are reviewed and accepted by S0.

## VRC1-D009 — accept R1/R2 requalification and open R3/R4

Decision: accept R1 `PASS WITH DOC FIXES` and R2
`PASS WITH DOCUMENTED PLATFORM LIMITATIONS`. The repaired semantic boundary is
qualified; the clean Linux x86-64 source-build/install and replay path is
qualified under its recorded emulation and unpublished-candidate limitations.
Close VRC1-F004 without erasing its history. Authorize R3 to correct only the
three recorded documentation issues plus independently observed first-user
friction, and authorize R4 to qualify the two bounded release-facing examples
without changing Vela Core. R5, R6, R7, versioning, tagging, publishing,
pushing, signing, and release remain unauthorized.

## VRC1-D010 — accept R3/R4 and open R5/R6

Decision: accept R3 and R4 as `PASS WITH DOCUMENTED LIMITATIONS`. Their
limitations are non-blocking at this phase: the candidate remains unpublished,
disposable macOS candidate qualification and R7 are pending, and the fixture
shell scripts temporarily install then remove public non-authority trust pins.
The merged release-facing surface is bound by Protocol root
`sha256:553c2bf5b495506e5297027c47abd68e058f1a34136900fc4e4606c81d311a17`.
Authorize R5 to audit semantic product legibility and implement only
semantically misleading or release-blocking corrections. Authorize R6 to audit
packaging and release integrity without tagging, publishing, pushing, signing,
or bumping a version. R7 remains blocked.
