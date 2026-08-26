# Append-only results ledger

## VC1-R000 — Baseline qualification

Date: 2026-08-26.

Protocol tests, selected complete lifecycle CLI tests, and the current
conformance runner passed at baseline HEAD
`23c2eb86b0deb1b155807fae16bcd7ba5bb707c0`. This establishes an initially
healthy repository and confirms many existing Level-1 semantics. It does not
establish campaign completion, cumulative handoff value, external validation,
or an anomaly.

```text
Campaign anomaly: NONE
Scientific experiments run: 0
```

## VC1-R001 — T2 replay and receipt qualification

Date: 2026-08-26.

T2 qualified deterministic scientific-state replay over the existing kernel and
made the boundary between exact state reconstruction and source-owned native
reruns explicit. The supervisor reproduced the new clean-clone Artifact and
Review Method test and found one integration defect: the new report was absent
from the required root documentation index. T2 corrected that defect in a
strict-descendant commit; the supervisor then reproduced the documentation
contract at 11/11 PASS and integrated the lane.

```text
Worker commits: a7d78de1c5ae8026378eca6088a8b5aefc2c0711, 415a335546646a8799ab5bf18ed8f51f6ee92312
Integrated commits: 29e19c00, b68f0b01
Semantic changes: none
Protocol objects changed: none
New replay engine or runner: none
Supervisor disposition: MERGE AFTER BOUNDED FIX
Campaign anomaly: NONE
Scientific experiments run: 0
```

The qualification proves exact reconstruction and refusal boundaries for
retained governed state. It does not prove native computational replay,
physical replication, cumulative handoff value, or external validation.
