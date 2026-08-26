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

## VC1-R002 — T1 kernel and conformance qualification

Date: 2026-08-26.

T1 qualified the existing governed-transition kernel without production or
wire changes. The worker added end-to-end coverage for two scoped passing
Verification Records, contradictory pass/fail evidence, incomplete and blocked
acceptance refusal, accepted-Standing invariance before Decision, exact
Verification-set consumption, and accepted retraction Event linkage.

The supervisor audited the complete diff, confirmed that the source-file edit
was confined to the existing test module, reproduced both modified CLI
lifecycles, and reproduced the root documentation contract. After T1 and T2
integration, the combined branch also passed all modified lifecycles and the
full Protocol 1 conformance runner with its root unchanged.

```text
Worker commit: 5cf684e0ae33865bfeccf43573676e775a399535
Integrated commit: d7bf4db2
Production semantic changes: none
Protocol or schema changes: none
Supervisor disposition: MERGE
Combined Protocol 1 root: sha256:e7a6d288918692d6a6186cc3e612871f167ba954c4cc31de28cce182a66a0afd
Campaign anomaly: NONE
Scientific experiments run: 0
```

This is Level-1 internal protocol qualification. It is not a cumulative-science
result, external validation, release qualification, or anomaly.
