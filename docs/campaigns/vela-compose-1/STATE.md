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

## State 0004 — Phase 1 qualified; T3 authorized

```text
Date: 2026-08-26
Phase: PHASE_2_COUNTERFACTUAL_APPARATUS
Supervisor branch: campaign/compose1-supervisor
Canonical integrated commit before state receipt: d7bf4db2
Foundational top-down search: CLOSED
Campaign anomaly: NONE
```

T1 and T2 are integrated and qualified. Combined supervisor verification
passed the modified CLI lifecycles, root documentation contract, and full
Protocol 1 conformance with the frozen root unchanged. T3 is authorized as the
only next primary lane. T4–T7 remain blocked. Scientific experiments executed:
none.

## State 0005 — T3 launched from the qualified Phase-1 root

```text
Date: 2026-08-26
Phase: PHASE_2_COUNTERFACTUAL_APPARATUS
Supervisor branch: campaign/compose1-supervisor
T3 starting commit: 9eff75e62319b766d33118cea71c1baa65e62d81
T3 branch: campaign/compose1-counterfactual
T3 task: 01a03d1c-d733-7ac2-a65c-8ae70f7e9fa9
T3 worktree: /Users/williamblair/.codex/worktrees/cde5/vela
Foundational top-down search: CLOSED
Campaign anomaly: NONE
```

T3 is active and attached to the exact qualified Phase-1 root. It is limited to
source-owned branching, sealed evaluation, deterministic comparison, and
honest metering qualification. T4–T7 remain blocked. Scientific experiments
executed: none.

## State 0006 — T3 qualified; T4 source selected

```text
Date: 2026-08-26
Phase: PHASE_3_LEAN_VERTICAL
Supervisor branch: campaign/compose1-supervisor
Canonical integrated commit before state receipt: a5f5d9b3
T4 source repository: /Users/williamblair/personal/lean-proofs
T4 source commit: 06d1322e62aa28b860da1ec66465d913c1902c78
T4 target: Erdos154.erdos_154_sumset
Lean: leanprover/lean4:v4.29.1
Mathlib commit: 5e932f97dd25535344f80f9dd8da3aab83df0fe6
Lake manifest SHA-256: f4c3e1fea9e745548c15b78b91015489277625c3dee15ab1ebe8bf6acf57b320
Foundational top-down search: CLOSED
Campaign anomaly: NONE
```

T3 is integrated with disposition `MERGE AFTER BOUNDED FIX`. The proposed T4
source checkout is clean, the exact target builds under its pinned environment,
and its axiom audit reports only `propext`, `Classical.choice`, and
`Quot.sound`. T4 is authorized; T5 substantive execution, T6, and T7 remain
blocked. Scientific experiments executed: none.

## State 0007 — T4 Lean vertical launched

```text
Date: 2026-08-26
Phase: PHASE_3_LEAN_VERTICAL
Supervisor branch: campaign/compose1-supervisor
Canonical T4 task: 01a03d3f-a006-7c51-85cd-b3392703f581
Canonical T4 workspace: /Users/williamblair/Documents/Codex/2026-08-26/vela-compose-1-lean-vertical
Duplicate task stopped: 01a03d41-7bbe-74b1-baac-7f1d670c3d03
Foundational top-down search: CLOSED
Campaign anomaly: NONE
```

The canonical T4 worker is active in a source-owned, projectless workspace and
is bound to the exact Vela binary and frozen Lean source inputs recorded in
State 0006. A task-registry lag caused one duplicate task to be created. The
supervisor ordered that duplicate to stop before outcome-bearing Lean execution
and to record no scientific conclusion. T1–T3 are complete and idle. T5
substantive execution, T6, and T7 remain dependency-gated; they are not active.

## State 0008 — T4 qualified; T5 independent review active

```text
Date: 2026-08-26
Phase: PHASE_4_ALZHEIMER_VERTICAL_QUALIFICATION
Supervisor branch: campaign/compose1-supervisor
T4 source commit: 05b6e36fb46b840eeac533658faf6f71ad99dc06
T4 terminal Vela root: sha256:1f18d90faec38dfb602d1f6bfa51c0f7eb69373698baeb4e8f73cbf5dba5c82c
T5 planning task: 01a03d55-755b-7090-a83c-80acd68642eb
T5 planning commit: b0eb6fc26c8deba2260a1326f5caf6a99153a2b2
T5 independent review task: 01a03d6a-efbf-7e92-9c73-c7328f9ff7e8
Foundational top-down search: CLOSED
Campaign anomaly: NONE
```

T4 qualifies as a Level-1 real verifier-rich vertical. Its v1 refusal remains
preserved; v1.1 used the documented withdrawal path and required no Core
change. T5 planning selected one bounded longitudinal CSF sPDGFRbeta/APOE4/A-T
question but created no scientific lifecycle or Standing. Only independent
field-level primary-source review is active. T5 execution, T6, and T7 remain
blocked on the next recorded supervisor gates.

## State 0009 — T5 v1 extraction rejected; bounded correction active

```text
Date: 2026-08-26
Phase: PHASE_4_ALZHEIMER_VERTICAL_QUALIFICATION
Supervisor branch: campaign/compose1-supervisor
T5 v1 planning commit/tree: b0eb6fc26c8deba2260a1326f5caf6a99153a2b2 / 9368d8ff58a771f9b410a97d9683ecf91e70dad6
T5 independent-review commit/tree: 5eb13c9bec3ecef051682b664ec6e4fa35f63491 / 587a8f3470a39f24a75cb7192d265193dfeaa064
T5 review verdict: REJECT_EXTRACTION
T5 scientific lifecycle: BLOCKED
T6 planning task: 01a03d6f-46ae-70e1-8873-902823313807
Foundational top-down search: CLOSED
Campaign anomaly: NONE
```

The supervisor reproduced the T5 review from a fresh clone, verified its
artifact hashes, and accepted the rejection. The source representations are
byte-stable; the failure is semantic. Montagne 2020 reports separate
Aβ1-42-adjusted and pTau-adjusted post hoc models, not a simultaneous A+T model,
and stratified significance does not establish formal APOE effect modification.
The v1 packet remains immutable. A strict-descendant v1.1 correction is active;
it may narrow the extraction and provenance labels but may not execute a Vela
scientific lifecycle. T6 remains at pre-execution task-design qualification.

## State 0010 — T6 candidate C selected; Stage 0 authorability authorized

```text
Date: 2026-08-26
Phase: PHASE_5_HANDOFF_APPARATUS_QUALIFICATION
Supervisor branch: campaign/compose1-supervisor
T6 design commit/tree: 32fbe541da65b3ffa6e73d4d77d866b34aa75eca / 4f1d0d26f94e12ffc5988806b8e30b2caa2dcbc4
T6 selected task: t6-erdos154-finite-uniformity-v1
T6 Stage 0: AUTHORIZED
T6 Stage 1 participant cells: BLOCKED
T5 v1.1 correction: ACTIVE
Foundational top-down search: CLOSED
Campaign anomaly: NONE
```

The original modulus-two continuation and candidates A/B were killed before
any model or Lean run because they reduce to direct theorem instances or two
limit subtractions. Candidate C survives as a bounded finite-aggregation
consequence. The supervisor reproduced internal/external design validation,
three equivalence/tamper tests, exact source and vertical bindings, target-name
absence, Git integrity, and fresh-clone validation. This authorizes only a
separate hidden evaluator-authorability and apparatus gate. It does not
authorize R/E/V participant cells or claim useful handoff value.

## State 0011 — T5 v1.1 extraction passed; lifecycle authorized

```text
Date: 2026-08-26
Phase: PHASE_4_ALZHEIMER_VERTICAL_EXECUTION
Supervisor branch: campaign/compose1-supervisor
T5 v1.1 planning commit/tree: 337f8bd836b06474fbbf00be6275b673a950ff7a / f8eb8b1c46ee07f6048098f43019e65bee64d9fb
T5 v1.1 review commit/tree: 427e234c77ebea6022ca835c0ee236e2c1ab0110 / 38cfa8925c123f27ad002cd23d4e483584d46060
T5 review verdict: PASS_EXTRACTION
T5 lifecycle experiment: VC1-E004 AUTHORIZED
T6 Stage 0: ACTIVE
Foundational top-down search: CLOSED
Campaign anomaly: NONE
```

The supervisor reproduced the corrected packet and independent re-review from
fresh clones. All twelve source representations match, the four Montagne rows
and separate model roles are exact, v1 artifacts remain immutable, and no
material mismatch remains. Two nonmaterial advisories become mandatory future
Submission fields: Preis QAlb `n=33` and source-specific absence of multiplicity
correction in Edwards 2025. A new source-owned T5 lifecycle may now execute the
frozen literature-report transition and clean-machine continuation. It may not
broaden the Claim, add sources, or create a biological or medical conclusion.

## State 0012 — T6 v1 fails Stage 0; syntax-only successor authorized

```text
Date: 2026-08-26
Phase: PHASE_5_HANDOFF_APPARATUS_QUALIFICATION
Supervisor branch: campaign/compose1-supervisor
T6 v1 design commit/tree: 32fbe541da65b3ffa6e73d4d77d866b34aa75eca / 4f1d0d26f94e12ffc5988806b8e30b2caa2dcbc4
T6 hidden-evaluator commit/tree: 57e7ea008c57df5343f10e0d75ab141046d7fad8 / cb371cb58b71a8aa264781e9ab93e50ee595fcaa
T6 apparatus commit/tree: 461c79df4be504b4d0071eb1dd1e90749a2b0f09 / b195cf458e6d827a71bd236867dffbfa0c935b5b
T6 v1 Stage 0: EXPERIMENTAL_APPARATUS_FAILURE
T6 scientific denominator: 0
T6 v1.1 design correction: AUTHORIZED
T6 Stage 1 participant cells: BLOCKED
T5 lifecycle: ACTIVE
Foundational top-down search: CLOSED
Campaign anomaly: NONE
```

Two independent Stage-0 paths reproduced the exact frozen target's Lean parse
failure at line 15:19: the statement uses `∑ i in Finset.range m` under pinned
Lean 4.29.1, which expects the current bounded-binder syntax. The supervisor
reproduced the diagnostic from the exact target SHA-256
`c415ff6e7e0d592d80ed28072276948f2ae61ad7906cb3ba69b61b8ec57ce3a5`,
reran all eleven apparatus tests in place and from a fresh clone, and verified
the design, Lean, T4, and Vela source trees remained clean. No participant,
model, or scientific cell started. Candidate C v1 is closed without a
scientific conclusion. A strict-descendant v1.1 may change only the malformed
binder spelling and reconcile the already-disclosed R Git-metadata packaging
contract. It may not change the proposition, task class, arms, order, budgets,
metrics, thresholds, reference custody, or exact-tokenizer gate. Stage 1
remains blocked pending a new hidden authorability PASS, apparatus PASS, exact
runtime-tokenizer qualification, and a separate S0 decision.

## State 0013 — T5 governed lifecycle passed; clean successor authorized

```text
Date: 2026-08-26
Phase: PHASE_4_ALZHEIMER_VERTICAL_CONTINUATION
Supervisor branch: campaign/compose1-supervisor
T5 lifecycle commit/tree: 363d1210e33f951739b6281097054f179ee04123 / ba7c3381899c76ae972a71ed7355c3e1fcfc087c
T5 lifecycle public receipt SHA-256: 1bc4b26523b28b4437ae2b0e47e4cbc79aae3bcf3f9719d8afc9c493aa30a1b9
T5 terminal Repository root: sha256:785fa897ac8ffa9e8dd92756090923ee9e8ce3ec593ed07bb73baa63aa58a79a
T5 lifecycle: PASS
T5 clean-successor task: AUTHORIZED
T6 v1.1 design correction: ACTIVE
Foundational top-down search: CLOSED
Campaign anomaly: NONE
```

The source-owned T5 repository completed the exact procedural-frontier to
reviewed-literature supersession lifecycle with no Core or schema change. The
initial Claim remains resolvable as superseded; the sole current accepted
Standing is the exact v1.1 reviewer-authorized Claim at root
`sha256:9fd44b6ab676e4234728ceeb49248911e08982f075c78474a6aa17c0b8467a24`.
Both scoped passing Verifications produced zero accepted-event delta. Only two
fresh-root authorized Decisions changed Standing. The supervisor reproduced
custody validation, strict replay, terminal counts and root, Git integrity, and
fresh-clone reconstruction. The separate clean-successor evaluator key is
sealed at commit/tree `affe34ba5933865f1a8705e40285131a29ec7b33` /
`687b1a1c56f663fe7f721a167d18e10b90edace4`. One blind successor may now
receive only a fresh clone, the exact binary, and the frozen participant task;
it has no answer key, prior conversation, network, Verification, Decision, or
Standing authority.

## State 0014 — T6 v1.1 design qualified; Stage 0 reauthorized

```text
Date: 2026-08-26
Phase: PHASE_5_HANDOFF_APPARATUS_QUALIFICATION
Supervisor branch: campaign/compose1-supervisor
T6 v1.1 design commit/tree: 169fd94724048c7d30d780ccceb3f3c7b38b18eb / 6a6da4dac8a0085bd5bbf5fbbd9c9d0d98cd7120
T6 v1.1 target SHA-256: aa18f3bb86b194c4ec1b481a803eb11b183f3c0f3ccefbd1e971695a19b225fb
T6 v1.1 Stage 0: AUTHORIZED
T6 v1.1 Stage 1 participant cells: BLOCKED
T5 clean successor: ACTIVE
Foundational top-down search: CLOSED
Campaign anomaly: NONE
```

The v1.1 design is a direct child of the immutable v1 design and adds only a
versioned overlay. Every v1 path remains byte- and mode-identical. The corrected
target differs by exactly the one authorized binder replacement and its
proposition parses and elaborates under Lean 4.29.1 without any proof or hidden
reference access. The R package contract now unambiguously requires the same
ordinary full Git checkout and reachable metadata/history in the common input
for all arms. The supervisor reproduced original and v1.1 internal/external
validation, three original tests, five correction tests, Git integrity, exact
ancestry, and a fresh `--no-local` clone rerun. Separate v1.1 hidden-authorability
and apparatus tasks may run Stage 0 only. Exact tokenizer identity and the <=5%
token-equivalence threshold remain mandatory and unevaluated; no participant or
scientific cell is authorized.
