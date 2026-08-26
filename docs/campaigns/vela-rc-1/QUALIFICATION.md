# VELA-RC-1 qualification matrix

| Gate | Status | Evidence | Blocking condition |
| --- | --- | --- | --- |
| Semantic integrity | `PASS WITH DOC FIXES` | independent `R1_REQUALIFICATION.md`, audit-only shipped-CLI matrix, portable verifier, full Core union, clippy | three bounded documentation corrections remain for R3 |
| Protocol conformance | `PASS ON REPAIRED CANDIDATE` | independent R1 and S0 reproduced root `sha256:6a9d475c...` and complete Core union | hosted exact-tree release rerun remains a later release-integrity requirement |
| Clean install | `PASS WITH DOCUMENTED PLATFORM LIMITATIONS` | independent `R2_REQUALIFICATION.md`: exact candidate source build/install and operator loop in pristine Linux x86-64 guests | disposable macOS not tested; candidate is not yet a signed release artifact |
| Replay | `PASS` | independent R1 trust matrix; R2 public Math and neutral clean-clone replay; missing/corrupt Artifact and pin failures | release packaging must preserve the qualified source identity |
| Docs / first user | `PASS WITH DOCUMENTED LIMITATIONS` | `R3_FIRST_USER_QUALIFICATION.md`; 14 documentation contracts; focused workflows; portable conformance | blind external user pending; public installer still targets ancestor v0.977.4 |
| Cross-domain examples | `PASS WITH DOCUMENTED LIMITATIONS` | `R4_EXTERNAL_FIXTURES.md`; formal failure/correction lifecycle; heterogeneous two-check lifecycle; independent clean-clone replay | candidate not yet a signed artifact; trust-pin cleanup after uncatchable termination is manual |
| Product legibility | `HOLD — PRODUCT SEMANTICS` | `R5_PRODUCT_LEGIBILITY.md`; Core/Workbench pass; current Problems projection inspected at exact deployment | legacy 0.977.3 projection is labelled current strict pass; agent Decisions counted as Human authority |
| Packaging | `HOLD — RELEASE INTEGRITY` | `R6_RELEASE_INTEGRITY.md`; traceability/reproducibility/signatures pass | archives omit project licenses and deterministic third-party notices; SPDX licensing fields are `NOASSERTION` |
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

## R3/R4 external-legibility gate

R3 and R4 are accepted with explicit limitations. Their merged release-facing
surface is independently bound by Protocol 1 root
`sha256:553c2bf5b495506e5297027c47abd68e058f1a34136900fc4e4606c81d311a17`.
No normative Protocol file, object schema, authority rule, product command, or
version changed. These gates establish comprehensible documentation and two
reproducible cross-domain examples, not adoption or release authorization.
