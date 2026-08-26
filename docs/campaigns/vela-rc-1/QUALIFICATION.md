# VELA-RC-1 qualification matrix

| Gate | Status | Evidence | Blocking condition |
| --- | --- | --- | --- |
| Semantic integrity | `HOLD — SEMANTIC BLOCKER` | R1 report and independent S0 reproduction | shipped replay does not enforce the normative independent sequence-one trust anchor |
| Protocol conformance | `PASS AT PHASE 0` | portable verifier and complete locked Core union reproduced the frozen root and all current suites | independent R1 semantic matrix still pending |
| Clean install | `PASS WITH DOCUMENTED PLATFORM LIMITATIONS` | R2 clean signed install and exact-candidate source build in pristine Debian x86-64 guests | disposable macOS not tested; R1 blocker remains independent |
| Replay | `TECHNICAL PASS; SEMANTIC HOLD` | T4/T5 roots plus neutral clean-clone fixture and corrupt-Artifact refusal | unpinned read succeeds contrary to normative Protocol 1 |
| Docs / first user | `BLOCKED` | awaits R1/R2 | contradiction or release-blocking ambiguity |
| Cross-domain examples | `BLOCKED` | awaits R1/R2 | B8 |
| Product legibility | `BLOCKED` | awaits R3/R4 | B9 |
| Packaging | `BLOCKED` | awaits R3/R4 | B10 |
| Blind user | `BLOCKED` | requires R1-R4 pass | objective task or semantic failure |

## Initial reproduction receipts

The baseline local debug CLI replayed T4 at commit/tree
`05b6e36fb46b840eeac533658faf6f71ad99dc06` /
`4b491446071efc4d6cd306397fa33e8b008e2f29` with Repository root
`sha256:1f18d90faec38dfb602d1f6bfa51c0f7eb69373698baeb4e8f73cbf5dba5c82c`.

It replayed T5 at commit/tree
`363d1210e33f951739b6281097054f179ee04123` /
`ba7c3381899c76ae972a71ed7355c3e1fcfc087c` with Repository root
`sha256:785fa897ac8ffa9e8dd92756090923ee9e8ce3ec593ed07bb73baa63aa58a79a`.

`uv run --project conformance --locked python conformance/verify.py` passed and
recomputed Protocol 1 root
`sha256:e7a6d288918692d6a6186cc3e612871f167ba954c4cc31de28cce182a66a0afd`.

These receipts establish continuity of current local evidence. They are not a
clean-install result, blind-user result, adoption result, or release approval.

The complete locked Core union subsequently passed from control commit
`6d680eebb4a17813e72b55685aa2eec6b34e5fae`. The release-candidate lint gate
`cargo clippy --locked --workspace --all-targets -- -D warnings` passed as
well. This closes the inherited-code regression check but does not pre-judge
R1's independent semantic audit or R2's clean-install gate.

## R1 gate

R1's report is [R1_SEMANTIC_AUDIT.md](R1_SEMANTIC_AUDIT.md). The supervisor
accepted `HOLD — SEMANTIC BLOCKER` after independently confirming the exact
contradiction. The current read path validates a self-consistent retained
authority history but does not independently select that history with the
local sequence-one pin required by Protocol 1. Because `status` can still emit
`integrity.strict: pass`, this is B1/B4/B6 release-blocking behavior rather
than an optional hardening improvement.

## R2 gate

R2's report is
[R2_CLEAN_INSTALL_QUALIFICATION.md](R2_CLEAN_INSTALL_QUALIFICATION.md). The
supervisor accepted `PASS WITH DOCUMENTED PLATFORM LIMITATIONS`. The signed
release install, public Math read, exact-candidate clean source build, complete
operator loop, frozen neutral replay bundle, and missing-Artifact negative path
all passed. Focused test `neutral_replay_fixture` passed under S0.

The qualified public fixture commits no standalone Protocol `standing_root`;
its accepted-set commitment is explicitly fixture-local. The clean consumer
also replayed before the external trust pin was installed, independently
reproducing R1's semantic blocker. Reproducible bytes therefore do not make the
current read result conforming.
