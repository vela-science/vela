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

## VC1-R003 — T3 counterfactual branching and metering qualification

Date: 2026-08-26.

T3 qualified controlled same-lineage counterfactual branches using Git plus the
existing Vela CLI. No production command, protocol object, schema, state
engine, workflow runner, or metering database was added. The fixture proves an
identical governed branch point, divergent authorized accept/reject histories,
bidirectional isolation, sealed task/evaluation/metering inputs, explicit
resource availability/comparability, clean terminal replay, and deterministic
comparison across fresh checkout paths.

The supervisor reproduced the positive lifecycle but found that the original
sealed JSON inputs were immutable yet advisory: the Rust test hardcoded their
semantics separately. T3 was returned. Its strict-descendant correction parses
and causally binds all three manifests, binds the evaluator to its exact source
digest, binds each metering receipt to the frozen plan root, and adds negative
manifest/implementation mismatch cases. The supervisor reproduced the amended
lifecycle and the repository documentation contract before integration.

```text
Worker commits: 78ce635b95999744b881d304c17f1bdbbaa58b7c, e3cbc31cf1bae2d50de1be8033c23879b719235c
Integrated commits: 344b048d, a5f5d9b3
Production or wire changes: none
Supervisor disposition: MERGE AFTER BOUNDED FIX
Campaign anomaly: NONE
Scientific experiments run: 0
```

The qualified comparator remains test-only and campaign-owned. It does not
establish value for a scientific vertical or authorize a public `vela compare`
surface.

## VC1-R004 — T4 Lean verifier-rich vertical qualification

Date: 2026-08-26.

T4 completed a real, source-owned Lean lifecycle using only general Vela APIs.
The initial natural proof attempt failed on the missing `IsSidon` to
`IsSidonSetNat` bridge; its Submission, exact Lean failure, failed Verification,
and later producer withdrawal remain addressable. The corrected published proof
then passed under the pinned Lean/Mathlib environment and reported only
`propext`, `Classical.choice`, and `Quot.sound`. That passing Verification
changed no Standing. Exactly one fresh-root authorized Decision admitted the
Claim and review Events, after which Standing contained one accepted corrected
Claim. A downstream specialization began from that exact Claim/root and remains
pending.

The supervisor independently reproduced the terminal repository root and all
counts both in place and from a fresh clone.

```text
Source repository: /Users/williamblair/Documents/Codex/2026-08-26/vela-compose-1-lean-vertical/work/t4-lean-repository
Source commit/tree: 05b6e36fb46b840eeac533658faf6f71ad99dc06 / 4b491446071efc4d6cd306397fa33e8b008e2f29
Terminal Vela root: sha256:1f18d90faec38dfb602d1f6bfa51c0f7eb69373698baeb4e8f73cbf5dba5c82c
Accepted Claims: 1
Pending Claims: 1
Withdrawn Proposals: 1
Submissions / Proposals / Verifications: 3 / 3 / 2
Core or wire changes: none
Supervisor disposition: MERGE (evidence reference only; vertical stays source-owned)
Campaign anomaly: NONE
```

This is Level-1 protocol evidence, not theorem discovery, general prover
capability, external validation, or cumulative-handoff value. The current CLI
still cannot author the downstream canonical `depends` edge, so the dependency
is exact source-owned evidence rather than a Claim relation. Exact zero-byte
verifier streams also require hash-receipt indirection.
