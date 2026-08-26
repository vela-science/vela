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
