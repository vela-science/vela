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

## State 0002 — T1 and T2 launched

```text
Date: 2026-08-26
Phase: PHASE_1_KERNEL_AND_REPLAY
Supervisor branch: campaign/compose1-supervisor
Canonical integrated commit: 6a0f5adee55f9c50e7e154ac8d118662809d3323
Foundational top-down search: CLOSED
Campaign anomaly: NONE
```

Active workers:

| Lane | Task ID | Branch | Worktree | State |
|---|---|---|---|---|
| T1 Kernel + Conformance | `01a03d07-4332-76f3-87b6-a1ed4cb5f259` | `campaign/compose1-kernel` | `/Users/williamblair/.codex/worktrees/c339/vela` | active from Phase 0 commit |
| T2 Receipts + Replay | `01a03d07-4332-76f3-87b6-a1c88d50efab` | `campaign/compose1-replay` | `/Users/williamblair/.codex/worktrees/a39a/vela` | active from Phase 0 commit |

T3 through T7 remain blocked by their frozen dependency gates. No scientific
experiment has started. The supervisor will inspect the exact branch diffs and
test receipts before any integration decision or downstream-lane launch.

## State 0003 — T2 integrated; T1 remains active

```text
Date: 2026-08-26
Phase: PHASE_1_KERNEL_AND_REPLAY
Supervisor branch: campaign/compose1-supervisor
Canonical integrated commit: b68f0b01
Foundational top-down search: CLOSED
Campaign anomaly: NONE
```

T2 is integrated with supervisor disposition `MERGE AFTER BOUNDED FIX`.
Deterministic governed-state reconstruction qualifies without adding a second
state engine, generic receipt ontology, or native runner. T1 remains active on
kernel conformance. T3 through T7 remain blocked until the complete Phase-1
gate is reviewed. Scientific experiments executed: none.
