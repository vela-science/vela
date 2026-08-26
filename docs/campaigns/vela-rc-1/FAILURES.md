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

No release blocker has yet been adjudicated. Absence of a known blocker is not
a passing gate.
