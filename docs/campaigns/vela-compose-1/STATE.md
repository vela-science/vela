# Append-only campaign state

Do not edit or delete prior entries. Append a new dated entry after every
meaningful integration gate.

## State 0001 — Phase 0 reconnaissance complete

```text
Date: 2026-08-26
Phase: PHASE_0_CONTROL_FREEZE
Supervisor branch: campaign/compose1-supervisor
Supervisor baseline HEAD: 23c2eb86b0deb1b155807fae16bcd7ba5bb707c0
Canonical integrated commit: PENDING_PHASE_0_COMMIT
Foundational top-down search: CLOSED
Campaign anomaly: NONE
```

Active workers: none until the Phase 0 control commit is reviewed.

Reserved branch map:

| Lane | Branch | Launch state | Preferred ownership |
|---|---|---|---|
| S0 | `campaign/compose1-supervisor` | active | campaign controls and integration only |
| T1 | `campaign/compose1-kernel` | pending Phase 0 commit | protocol/event/object conformance and kernel lifecycle tests |
| T2 | `campaign/compose1-replay` | pending Phase 0 commit | replay/receipt qualification without core semantic changes |
| T3 | `campaign/compose1-counterfactual` | blocked on T1/T2 | source-owned branch apparatus and metering |
| T4 | `campaign/compose1-lean` | blocked on stable APIs | source-owned Lean vertical |
| T5 | `campaign/compose1-alzheimer` | planning blocked until repository selection | source-owned biological vertical |
| T6 | `campaign/compose1-handoff` | blocked on T3 plus vertical | preregistered R/V/E experiment |
| T7 | `campaign/compose1-release` | blocked on qualified behavior | integration/release only |

Frozen decisions: VC1-D001 through VC1-D005 in `DECISIONS.md`.

Experiments executed: none. Invalid/excluded experiments: none. Results:
baseline engineering checks only. Next authorized actions: commit Phase 0, create
T1/T2 branches/worktrees from that commit, and launch only T1/T2.

