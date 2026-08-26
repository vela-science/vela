# VELA-RC-1 failure ledger

Preserve every release blocker, documentation blocker, invalid qualification,
and important non-blocker. Do not remove an entry after a fix; append its
resolution and evidence.

## Open observations at freeze

| ID | Classification | Observation | Disposition |
| --- | --- | --- | --- |
| VRC1-F001 | `QUALIFICATION GAP — PARTIALLY CLOSED` | Hosted conformance is green for an ancestor, not exact RC-1 baseline `421cdc0d...` | Exact local union passed from RC-1 control commit; hosted exact-tree qualification still belongs to any later authorized release process |
| VRC1-F002 | `QUALIFICATION GAP` | No RC-1 clean-install path has yet been exercised without campaign-local state | R2 owns the test |
| VRC1-F003 | `ERGONOMIC / PACKAGING AUDIT INPUT` | Ignored local `dist/` contains stale v0.977.2 material while the source version is v0.977.4 | Never treat ignored residue as candidate evidence; R6 audits clean packaging |

## VRC1-F004 — independent authority selection absent from shipped replay

Classification: `RELEASE BLOCKER`.

Normative Protocol 1 requires strict replay to verify the independently pinned
sequence-one authority root. The shipped `replay`, `status`, and read paths use
the valid authority chain retained inside the Repository without consulting
the local pin; a current passing integration test removes the pin and still
expects replay and `integrity.strict: pass`. Decision writes correctly refuse
without the pin, but that separate safeguard does not cure read-side authority
selection.

Required disposition: `HOLD`. Do not weaken the normative trust model. A
bounded repair must make the public read/replay contract and implementation
agree, fail closed on missing or mismatched external selection, add shipped-CLI
negative coverage, and pass independent R1 requalification before downstream
gates open.

Repair status: `IMPLEMENTED; INDEPENDENT REQUALIFICATION PENDING`. Supervisor
commit `ad2a4516078525025d05bd461b550ed5b8e35971` implements the existing
normative trust selection across public governed-state reads. S0's direct
missing/malformed/mismatch/positive matrix, portable verifier, clippy gate, and
complete Core union pass. This entry remains open until fresh R1 and R2 audits
accept the exact repaired tree.

Resolution: `CLOSED — INDEPENDENTLY REQUALIFIED`. R1 returned
`PASS WITH DOC FIXES`; R2 returned
`PASS WITH DOCUMENTED PLATFORM LIMITATIONS`; S0 integrated their immutable
reports and independently reran the direct trust matrix and neutral fixture.
The closure is specific to repaired product commit
`ad2a4516078525025d05bd461b550ed5b8e35971`; it does not authorize release.

## VRC1-F005 — clean-path documentation friction

Classification: `DOC BLOCKER INPUT`, subordinate to VRC1-F004.

R2 found that the write journey assumes Git author identity, while the
quickstart shows a Git commit without explicitly naming `user.name` and
`user.email` setup. The signed installer also assumes ordinary download,
archive, digest, OpenSSH, and writable-prefix tooling without a compact
platform prerequisite table. Source builds assume a compiler linker and
network access for missing pinned components/crates.

Required disposition: retain for R3 after G1 is repaired and requalified. Do
not open R3 while the semantic gate is on HOLD.

Gate update: R3 is now authorized and owns this documentation work.

Resolution: `CLOSED`. R3 added the missing prerequisites and exact executable
write contract; the focused first-user workflows and documentation contracts
pass. The existing signed public binary remains an ancestor and is assigned to
R6 rather than hidden by documentation.

## VRC1-F006 — Proposal-root catalogue overstates stored fields

Classification: `DOC FIX REQUIRED`.

`docs/ROOTS.md` says the Proposal root covers its canonical record "and
status", although `vela.proposal.v1` has no status field. Status is derived
from withdrawals and governed Decisions/Events. R3 must correct the catalogue
without changing object bytes or semantics.

Resolution: `CLOSED`. R3 now states that Proposal status is derived from
withdrawals and governed Decision Events.

## VRC1-F007 — release checklist requests a nonexistent Standing digest

Classification: `DOC FIX REQUIRED`.

`RELEASE_CHECKLIST.md` asks for an expected standalone "Standing digest" even
though Protocol 1 publishes no standalone `standing_root`. R3 must name the
actual commitments: accepted set, Repository root, and authority Event-log
root.

Resolution: `CLOSED`. The checklist now names the actual commitments.

## VRC1-F008 — implementer semantic matrix is campaign-local

Classification: `DOC FIX REQUIRED`.

The complete scenario matrix exists in reviewed campaign evidence but is not
yet a compact release-facing implementer index. R3 must publish or link an
equivalent matrix without making campaign prose normative.

Resolution: `CLOSED`. `conformance/README.md` now provides the informative
scenario-to-fixture index and preserves the distinction between verification
and acceptance.

## VRC1-F009 — public installer identifies the ancestor release

Classification: `PACKAGING / VERSION AUDIT INPUT`.

The signed public `v0.977.4` artifact and tag predate the RC-1 repaired
candidate even though the source version remains `0.977.4`. R3 verified the
ancestor installer but did not treat it as candidate evidence. R6 must decide
the exact release source identity and recommend KEEP VERSION, PATCH BUMP,
MINOR/PRE-1.0 BUMP, PROTOCOL BUMP, or HOLD without publishing anything.

Disposition update: R6 recommends `PATCH BUMP` to 0.977.5 after all release
blockers close. No bump is performed or authorized by RC-1 qualification.

## VRC1-F010 — legacy projection overstates strict integrity

Classification: `RELEASE BLOCKER — PRODUCT SEMANTICS`.

The current Problems deployment renders `strict pass` from a projection
generated by Vela 0.977.3, whose governed reads did not enforce the independent
sequence-one pin now required by Protocol 1. A bounded Vela Web repair must
make legacy/unqualified projection provenance visibly non-current or fail
admission, and the future projection builder must require independent trust
selection before emitting current strict integrity. No deployment is
authorized during qualification.

## VRC1-F011 — Decision count falsely labelled human-only

Classification: `RELEASE BLOCKER — SEMANTICALLY MISLEADING`.

The Problems Repository overview labels a retained Decision count `Human
authority` even though it includes agent-performed Decisions and the detail
surface correctly identifies them. Replace it with `Authorized Decisions` and
bind an agent-class regression test; do not redesign the page.

## VRC1-F012 — release archives omit qualifiable license/notice material

Classification: `RELEASE BLOCKER — RELEASE INTEGRITY`.

Both supported archives contain only the executable. Project license texts and
deterministic third-party notice material are absent; the SPDX inventories
cannot substitute because all relevant package fields are `NOASSERTION`.
Repair package contents and make smoke/reproducibility gates refuse omissions,
then independently inspect both platforms before release qualification can
pass.
