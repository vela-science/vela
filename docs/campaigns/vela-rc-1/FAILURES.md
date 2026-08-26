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
