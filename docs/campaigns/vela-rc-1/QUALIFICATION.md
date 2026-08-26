# VELA-RC-1 qualification matrix

| Gate | Status | Evidence | Blocking condition |
| --- | --- | --- | --- |
| Semantic integrity | `IN PROGRESS` | R1 not yet complete | Any B1, B3-B6, or B9 finding |
| Protocol conformance | `PARTIAL PASS` | portable verifier reproduced the frozen root | Full Core union pending |
| Clean install | `UNTESTED` | R2 not yet complete | B7 |
| Replay | `PARTIAL PASS` | T4/T5 exact roots reproduced locally | clean-environment public fixture pending |
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
