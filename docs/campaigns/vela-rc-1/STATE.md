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
