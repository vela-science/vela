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

## VC1-R005 — T5 v1 independent extraction rejection

Date: 2026-08-26.

The independent T5 reviewer retrieved and matched all twelve frozen PubMed XML
and PMC BioC representations byte-for-byte, then found material semantic and
provenance-label mismatches in the planning packet. Montagne 2020 used separate
Aβ1-42-adjusted and pTau-adjusted post hoc longitudinal models; it did not fit
one simultaneous A+T-adjusted model. Significant within-APOE4 estimates and
nonsignificant within-APOE3 estimates do not by themselves prove formal effect
modification. The S1/S2 cached PMC full-text representations are author
manuscripts, and the Preis source contains an internally inconsistent reported
F/df/p combination that cannot be decision-bearing.

```text
Planning commit/tree: b0eb6fc26c8deba2260a1326f5caf6a99153a2b2 / 9368d8ff58a771f9b410a97d9683ecf91e70dad6
Review commit/tree: 5eb13c9bec3ecef051682b664ec6e4fa35f63491 / 587a8f3470a39f24a75cb7192d265193dfeaa064
Review artifact hashes: PASS
Fresh-clone and Git-integrity audit: PASS
Review verdict: REJECT_EXTRACTION
Scientific lifecycle: BLOCKED
Scientific or medical conclusion: NONE
Campaign anomaly: NONE
```

This is a successful rejection by the campaign’s evidence gate, not a negative
result about blood-brain-barrier biology. The source corpus remains usable only
after a strict-descendant corrected extraction passes independent re-review.

## VC1-R006 — T6 pre-outcome task funnel

Date: 2026-08-26.

The direct modulus-two continuation and two nearby reformulations were rejected
before any participant/model or Lean process. Each collapses to direct use of
the accepted theorem plus elementary fixed-limit algebra and therefore cannot
measure inherited-state reconstruction or continuation cost. A fixed third
candidate, finite uniformity aggregate, survives only to Stage 0 because it
adds indexed theorem instantiation, absolute-value continuity, finite limit
aggregation, coercion normalization, and final algebra.

```text
Design commit/tree: 32fbe541da65b3ffa6e73d4d77d866b34aa75eca / 4f1d0d26f94e12ffc5988806b8e30b2caa2dcbc4
Pre-outcome kills: modulus-two, A, B
Selected pending task: C / t6-erdos154-finite-uniformity-v1
Participant/model starts: 0
Lean process starts: 0
Scientific denominator: 0
Stage 0: AUTHORIZED
Stage 1: BLOCKED
Campaign anomaly: NONE
```

This is apparatus/task qualification, not evidence that Vela improves
continuation and not a cumulative-intelligence result.

## VC1-R007 — T5 v1.1 extraction qualification

Date: 2026-08-26.

The corrected T5 packet passed independent primary-source re-review. All twelve
PubMed/BioC representations matched, the decisive Montagne supplement was
re-rendered and checked, all four model rows were exact, and the Claim boundary
correctly separates Aβ1-42 and pTau adjustment while leaving formal APOE effect
modification, simultaneous A+T adjustment, replication, and baseline
A-negative/T-negative persistence unestablished.

```text
Planning commit/tree: 337f8bd836b06474fbbf00be6275b673a950ff7a / f8eb8b1c46ee07f6048098f43019e65bee64d9fb
Review commit/tree: 427e234c77ebea6022ca835c0ee236e2c1ab0110 / 38cfa8925c123f27ad002cd23d4e483584d46060
Review verdict: PASS_EXTRACTION
Material residual mismatches: 0
Nonmaterial advisories: Preis QAlb n=33; Edwards source-specific no multiplicity correction
Supervisor fresh-clone/hash/ancestry audit: PASS
Scientific Standing created: 0
Lifecycle VC1-E004: AUTHORIZED, NOT YET RUN
Campaign anomaly: NONE
```

This result qualifies the extraction for a governed lifecycle. It does not
establish a biological effect, clinical conclusion, or accepted Standing.

## VC1-R008 — T6 v1 Stage-0 apparatus failure

Date: 2026-08-26.

The exact candidate-C v1 target is not parseable under the pinned Lean 4.29.1
environment. Its bounded sum uses `∑ i in Finset.range m`; both the independent
hidden evaluator and the apparatus worker reported `unexpected token 'in';
expected ','` at line 15:19. The supervisor independently reproduced that
diagnostic from the exact frozen target hash. No reference proof was exposed,
and no participant, model, or scientific Lean cell started.

The failure does not measure R, E, V, proof ability, continuation cost, or
governed-state value. Other Stage-0 components qualified, including
deterministic package roots, 24/24 V/E scientific fields and 11/11 evidence
blobs, native V replay, exact binary custody, sacrificial containment, and
eleven fail-closed tests. Exact runtime tokenizer identity remains unavailable
and is still a mandatory blocker rather than a proxy count.

```text
Hidden-evaluator commit/tree: 57e7ea008c57df5343f10e0d75ab141046d7fad8 / cb371cb58b71a8aa264781e9ab93e50ee595fcaa
Hidden authorability receipt SHA-256: 54f90a2dc24d0cdd0ea8adc654a70ab8f2272cb6132f4e0b7ce200cda53d1c7c
Apparatus commit/tree: 461c79df4be504b4d0071eb1dd1e90749a2b0f09 / b195cf458e6d827a71bd236867dffbfa0c935b5b
Apparatus receipt root: sha256:50c80085fdf4ba3c4902e1b16fea3e383dc5062985e608f7ebeaa183deb2ff8c
Scientific denominator: 0
Stage-0 disposition: EXPERIMENTAL_APPARATUS_FAILURE
Stage-1 status: BLOCKED
Scientific conclusion: NONE
Campaign anomaly: NONE
```

Candidate C v1 is terminal and immutable. VC1-D014 permits only a versioned
syntax-level apparatus correction; it does not authorize participant cells or
reinterpret this failure as evidence for or against cumulative handoff.

## VC1-R009 — T5 governed Alzheimer literature lifecycle

Date: 2026-08-26.

VC1-E004 completed the frozen source-owned lifecycle using only current general
Vela APIs. An initial procedural unresolved-frontier Claim was submitted,
independently verified within its narrow source-review scope, and accepted by a
fresh-root Decision. The exact corrected v1.1 literature-report Claim then
superseded it through a second scoped Verification and second fresh-root
Decision. The initial Claim remains retained as superseded. The sole current
accepted Claim is the exact reviewer-authorized statement; it preserves
separate Aβ1-42/pTau model roles, the Preis and Edwards advisories, source-family
dependence, all missingness, and every explicitly unestablished proposition.

Both Verifications recorded zero accepted-event delta. Only Decisions changed
Standing. Strict status/replay, object and receipt resolution, correction
lineage, custody hashes, Git integrity, and fresh-clone reconstruction passed.
One failed relative-output Verification invocation was preserved and changed
no state; the identical scoped invocation from the Repository directory then
succeeded. No Core, wire, schema, biological special case, source retrieval,
effect estimate, medical claim, outreach, or publication occurred.

```text
Source repository: /Users/williamblair/Documents/Codex/2026-08-26/vela-compose-1-t5-alzheimer-lifecycle/outputs/vc1-e004-alzheimer-governed-lifecycle
Terminal commit/tree: 363d1210e33f951739b6281097054f179ee04123 / ba7c3381899c76ae972a71ed7355c3e1fcfc087c
Public receipt SHA-256: 1bc4b26523b28b4437ae2b0e47e4cbc79aae3bcf3f9719d8afc9c493aa30a1b9
Initial accepted Repository root: sha256:790e6e45d1c74dd66189451f16cb83e71acb9b183c4ba4c14255eb7fb9962286
Terminal Repository root: sha256:785fa897ac8ffa9e8dd92756090923ee9e8ce3ec593ed07bb73baa63aa58a79a
Terminal accepted Claim root: sha256:9fd44b6ab676e4234728ceeb49248911e08982f075c78474a6aa17c0b8467a24
Current accepted Claims / Submissions / Verifications: 1 / 2 / 2
Accepted reviews / Vela artifacts / Events: 2 / 10 / 5
Core or wire changes: none
Lifecycle disposition: PASS
Clean successor: AUTHORIZED, NOT YET RUN
Campaign anomaly: NONE
```

This qualifies one real-science governance lifecycle. It does not yet qualify
blind continuation, cumulative handoff value, or a release claim; those remain
separate gates.

## VC1-R010 — T6 candidate-C v1.1 design correction

Date: 2026-08-26.

The T6 design repository now contains an additive v1.1 overlay that preserves
every v1 path unchanged. The target differs from the terminal malformed v1
bytes by exactly one replacement: `∑ i in Finset.range m` becomes the current
Lean membership-binder spelling `∑ i ∈ Finset.range m`. A proof-free
proposition check parses and elaborates under the frozen Lean 4.29.1 source
environment. The v1.1 package contract also resolves the already-disclosed
wording contradiction by requiring the full ordinary reachable Git metadata
and history in the common checkout for all three arms.

```text
Design commit/tree: 169fd94724048c7d30d780ccceb3f3c7b38b18eb / 6a6da4dac8a0085bd5bbf5fbbd9c9d0d98cd7120
Direct v1 parent: 32fbe541da65b3ffa6e73d4d77d866b34aa75eca
Corrected target SHA-256: aa18f3bb86b194c4ec1b481a803eb11b183f3c0f3ccefbd1e971695a19b225fb
Manifest SHA-256: 5610338a182d9eba89cda34eeced328917b928c2146e1c5885d2693c434e6fe5
Diff receipt SHA-256: 5e828cba203b48b76d44c750a14ba71ff9809d331f9eb027c722fc5d699d0836
Unchanged-control receipt SHA-256: 073c10e1559649db5bb007379a2f68d323b97f7da6f8f42f6327069f05b92100
Original and v1.1 validation/tests, fresh clone: PASS
Participant/model/scientific starts: 0
Scientific denominator: 0
Stage 0: AUTHORIZED
Stage 1: BLOCKED
Campaign anomaly: NONE
```

This is an apparatus correction, not handoff evidence. Hidden proof
authorability, package/runtime qualification, and exact tokenizer equivalence
remain unresolved.

## VC1-R011 — T5 clean-successor bounded failure

Date: 2026-08-26.

The one frozen blind successor run failed its clean-continuation gate. It
validated the exact lifecycle commit and Vela binary, began reading committed
state, and produced descriptive answers, but scientific tool call four emitted
approximately 18,789 stdout tokens before transport truncation. That exceeded
the frozen 15,000-observable-token cap. The participant stopped at 80 seconds,
performed no retry, and did not execute `vela submit`; therefore it created no
pending continuation Proposal.

The run made no canonical mutation. The supervisor verified one current
accepted Claim, two accepted review Decisions in history, five unchanged
Events, an empty Inbox, strict replay PASS, and the unchanged Repository root.
Only run receipts were added to the fresh clone.

```text
Successor task: 01a03dac-9eaa-7f82-966a-0313432dd260
Run commit/tree: f16d4ef929e16d5ee2510c77d621b9b234d29e76 / b08e8143116e797fe80bd810acfe907b9e2b43d6
RESULT.md SHA-256: aaf1bfce20e8ced3a695efcf0c63068669b51b367978f26aa1726b9b0e78cd84
RUN_REPORT.json SHA-256: febb2f5254bcb2475caf1ab5cc2f720a5c156235079174025aeaabe94261b498
Elapsed / scientific tool calls: 80 seconds / 4
Approximate trigger-call stdout tokens: 18,789
Pending Proposals: 0
Standing or Event changes: 0
Clean-successor disposition: FAIL
Retry: NONE AUTHORIZED
Campaign anomaly: NONE
```

This result preserves the narrower T5 lifecycle qualification but rejects the
stronger claim that the frozen package demonstrated efficient blind
continuation. It is not evidence that such continuation is impossible in
general.

## VC1-R012 — T6 v1.1 Stage-0 terminal failure without scientific data

Date: 2026-08-26.

Candidate C did not qualify for participant execution. The hidden evaluator
applied the single authorized binder correction to the sealed v1 reference
candidate and verified that every proof-body byte remained unchanged. The
corrected candidate nevertheless failed Lean at line 27:17; exact declaration
type, introduced declarations, and axiom-set gates therefore did not run. The
failure is an authorability failure of the frozen reference artifact, not a
prover or treatment outcome.

The independent public apparatus passed deterministic two-build package roots,
byte-identical common Lean checkout and Git history across R/E/V, all 24 V/E
scientific fields and 11 evidence blobs, native Vela status/why/replay, exact
frozen-binary recovery, containment/custody, tamper controls, and 11 automated
tests. It could not identify an exact tokenizer and version for the frozen
runtime. The preregistered exact token counts, syntax overhead, and <=5%
equivalence threshold therefore remain unevaluated; no proxy was substituted.

```text
Hidden-evaluator commit/tree: 75f3634443e69667b01768e3f195860df163f94f / f50adc3a2e3342abd11ec9bf3a5f4e52a9f92851
Authorability receipt SHA-256: 348d904b41ef34fb2046f2a91d58b2ac6f49023cd9ba512f8be9621b20676084
Authorability result: HIDDEN_EVALUATOR_AUTHORABILITY_FAIL
Apparatus commit/tree: a835bcc23b468e9588f170a13db17cafd8b10b5d / 97c3ad0b2760e8b34a50e86227ab920203203cf4
Apparatus receipt root: sha256:f774f8c4002e381bb5b47c9bedd5da47f50d98fe984515106d6baa0b3e449625
Apparatus result: BLOCKED_RUNTIME_IDENTITY
Supervisor fresh-clone tests: 4/4 evaluator; 11/11 apparatus
Participant/model/scientific starts: 0 / 0 / 0
Scientific denominator: 0
Stage 1: CLOSED WITHOUT EXECUTION
Scientific conclusion: NONE
Campaign anomaly: NONE
```

T6 therefore provides no evidence for or against cumulative handoff value.
The campaign may report only that the intended R/E/V screen was not executed
because its frozen Stage-0 gates failed. Candidate C is terminal and no further
correction or substitute target is authorized.
