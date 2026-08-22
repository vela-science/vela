# Final narrow independent re-review: EQ-R1 and EQ-R2

## Verdict

**PASS** for producer commit
`5fcb5567a329728a6c9e813546a3c97638b42e75`, tree
`46620b21fd9c980e33ee3c5257c121ccbae479af`, over parent
`5e01d3e07951d9d231dd28f31bad1af785da2837`.

This review is limited to EQ-R1, EQ-R2, and the requested identity gates. Both
previous blockers are closed. Every regular bundle file is rejected whenever
its link count is not one, including a second hardlink outside the bundle.
Permit consumption keeps the validated descriptor open and proves the linked
and final consumed device, inode, bytes, and link count across link and unlink.
The deterministic source-replacement adversary fails closed, and the concurrent
race retains exactly one byte- and inode-identical winner.

No producer byte was modified. This review called no provider or external
service, handled no credential, released no permit, and performed no merge,
release, authority, Decision, Standing, or scientific-record action.

## Exact binding and scope

- Producer ref: `origin/codex/evidence-harness-consolidation`
- Producer commit/tree:
  `5fcb5567a329728a6c9e813546a3c97638b42e75` /
  `46620b21fd9c980e33ee3c5257c121ccbae479af`
- Parent commit:
  `5e01d3e07951d9d231dd28f31bad1af785da2837`
- Prior BLOCKED review:
  `acc8d9945a91a81d478f8b6f89b705175d5f0faa`
- Reviewed at: `2026-08-22T04:11:35Z`

The refreshed remote equals the immutable producer commit and advertised tree.
The producer is the direct child of the stated parent. Its delta is exactly
three files, 174 insertions and 35 deletions:

| Path | SHA-256 at producer |
| --- | --- |
| `tools/evidence_qualification/README.md` | `sha256:5a78f54adfbb6e0c3dc4bb5d81edd72cb54dafde23025778a6be08db6dce0c34` |
| `tools/evidence_qualification/qualification.py` | `sha256:628ac203a48ef19c649dd64dedc010d104d728eb0edbb66392e93955fab872b9` |
| `tools/evidence_qualification/test_qualification.py` | `sha256:1259108e184571351f01a66fdc46f370b3b2077bb9ea481bbe83a65ab5327184` |

The clean detached producer checkout remained clean.

## EQ-R1: complete hardlink rejection

PASS.

`validate_bundle_tree` now requires `st_nlink == 1` for every regular file
encountered in the bundle. Both `read_regular` paths independently recheck link
count on the opened descriptor and again after reading, while preserving the
same device and inode. This covers aliases already present during the tree scan
and aliases introduced before or during a referenced-file read.

The committed regression creates a valid bundle, adds a second hardlink to
`schemas/registered.json` in a distinct temporary directory outside the bundle,
confirms link count two, and requires
`bundle_file_link_count_invalid`. Independent reproduction returned exactly:

```text
EQ-R1 blocked: bundle_file_link_count_invalid
```

The pre-existing in-bundle distinct-role hardlink regression now fails at the
same earlier link-count boundary. No hardlinked referenced byte received a
qualification receipt.

## EQ-R2: descriptor-, inode-, and byte-bound permit consumption

PASS.

`consume_permit` now keeps the validated source descriptor open from the first
no-follow open through completion. It:

1. requires a regular single-link source inode;
2. reads and validates the permit through that descriptor;
3. rechecks its device, inode, link count, size, and bytes;
4. creates the consumed hardlink without overwrite;
5. opens the consumed name no-follow and proves both descriptors identify the
   validated inode, have link count two, and retain the validated bytes;
6. separately opens and proves the source name still identifies those same
   bytes immediately before unlink;
7. unlinks the source; and
8. proves the consumed descriptor is the same inode and bytes with final link
   count one before returning success.

Any qualification mismatch after linking removes the consumed name and fails.
The deterministic review adversary replaces the source name immediately before
`os.link`. Independent reproduction returned:

```text
EQ-R2 blocked: permit_consumed_inode_or_bytes_mismatch;
consumed_absent=true; validated_bytes_retained=True
```

The eight-thread committed race produces exactly one winner. Its consumed file
has the original source device and inode, exact original bytes, and final
`st_nlink == 1`; the other seven attempts fail closed. This passed on every
reviewed Python minor.

## Requested checks

Fresh locked environments were created outside the repository and invoked
directly with `PYTHONNOUSERSITE=1` and `-s`:

| Python | Focused suite |
| --- | --- |
| 3.11.15 | 39/39 PASS |
| 3.12.10 | 39/39 PASS |
| 3.13.3 | 39/39 PASS |
| 3.14.4 | 39/39 PASS |

Additional requested gates:

- PASS: locked Ruff check for `tools/evidence_qualification`.
- PASS: locked Ruff format check; all five package files already formatted.
- PASS: parent-to-producer `git diff --check`.
- PASS: Protocol 1 verification: 77 normative and 39 informative files at
  `sha256:6ca327d6ec6f56c051e236be9eee42629e1f67dce393c1518e051ff8c14b279e`.
- PASS: byte-identical `docs/PROTOCOL.md`, `schemas/`, and `conformance/`
  across the corrective range.
- PASS: no diff under `paper/`; the historical 16-cell and 36-cell evidence,
  scores, and review lineage are unmodified.
- PASS: exact three-file scope and clean detached status.

## Claim ceiling

At this immutable commit, the maintained qualifier supports the narrow claims
reviewed here: every referenced regular bundle file must be single-link at its
validated read boundary, and successful permit consumption is bound to the
validated descriptor, inode, and bytes through the final single-link consumed
state. The previous EQ-R1 and EQ-R2 blockers require no further correction.

This PASS does not widen Protocol 1 or reinterpret historical evidence. The
qualifier remains non-authoritative tooling: it performs no provider call,
scientific session, participant-permit release, Repository authority action,
Decision, Event, or Standing transition. Builder independence remains an exact
receipt attestation rather than a build performed by this command.
