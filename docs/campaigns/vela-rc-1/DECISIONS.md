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
