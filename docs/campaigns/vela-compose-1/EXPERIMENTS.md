# Frozen experiment registry

No outcome-bearing empirical experiment is authorized in Phase 0 or the initial
T1/T2 engineering phase. Baseline conformance checks are engineering evidence,
not scientific cells.

Before any T4, T5, or T6 run, append a frozen entry containing exactly:

```text
Experiment ID:
Question:
Hypothesis:
Treatment:
Controls:
Task/data:
Model:
Tools:
Information available:
Resource budget:
Primary outcome:
Secondary outcomes:
Inclusion criteria:
Exclusion criteria:
Stopping rule:
Interpretation table:
Artifacts to preserve:
Frozen commit and hashes:
Execution authority:
```

Success criteria never change after outcome inspection. Invalid and excluded
runs remain preserved in attempted denominators.

## VC1-E001 — T4 Lean verifier-rich vertical v1

This central entry is a post-run mirror. The authoritative preregistration was
committed in the source-owned experiment repository before execution at
`3894a54b8f88f4e07d1520e10ce1f7f07ad44815`; the omission of a contemporaneous
central mirror is retained as a campaign-documentation defect and does not
rewrite that freeze.

```text
Experiment ID: VC1-E001 / T4-v1
Question: Can the current protocol retain a failed real Lean attempt and then relate a corrected Submission to that non-accepted candidate through --supersedes?
Hypothesis: The frozen relation attempt will either admit a non-distorting lifecycle or expose an exact semantic boundary.
Treatment: One natural failed proof, failed Lean Verification, and one corrected --supersedes Submission targeting the failed candidate Claim.
Controls: Frozen source/toolchain/artifact hashes; zero accepted Standing before and after Verification; no acceptance of the known-failing attempt.
Task/data: Erdos154.erdos_154_sumset at lean-proofs commit 06d1322e62aa28b860da1ec66465d913c1902c78.
Model: None; already-published proof fixture, no proof search.
Tools: Vela 0.977.4; Lean 4.29.1; Mathlib 5e932f97dd25535344f80f9dd8da3aab83df0fe6.
Information available: Frozen theorem, natural missing-bridge attempt, corrected published fixture, exact source environment.
Resource budget: One attempted Lean execution; stop before corrected execution if relation is refused.
Primary outcome: Whether the exact non-accepted candidate can be the correction/supersession target without changing predecessor Standing.
Secondary outcomes: Failed-evidence retention, accepted-Standing invariance, replay, exact output custody.
Inclusion criteria: Exact frozen hashes and expected natural Lean failure.
Exclusion criteria: Parser/binding invocations that create no Verification; all preserved.
Stopping rule: Stop on relation refusal, hash mismatch, unexpected Lean result, new axiom, replay mismatch, or required Core change.
Interpretation table: Admit = continue frozen lifecycle; refuse because target is non-accepted = qualified semantic boundary; any distorted acceptance = protocol failure.
Artifacts to preserve: Freeze, attempt bytes, exact streams/status, failed Verification, refusals, Event/Standing audit, replay.
Frozen commit and hashes: 3894a54b8f88f4e07d1520e10ce1f7f07ad44815; target SHA-256 9ac3fc83bbeba2df4739b5f3d69130876d99ea09c47d0c30977339904d74f457.
Execution authority: T4 task 01a03d3f-a006-7c51-85cd-b3392703f581; source-owned repository authority only.
```

Terminal source-owned commit: `d4c88ceb64738f29b804a4dc7e735272bd873dd5`.
Result: the relation was refused because the predecessor was not accepted.
Supervisor adjudication: the semantic boundary is valid, but the full-vertical
stop was an `EXPERIMENTAL_DESIGN_FAILURE` because pending-Proposal replacement
has a separate authenticated withdrawal path.

## VC1-E002 — T4 Lean verifier-rich vertical v1.1

The authoritative v1.1 freeze was committed before continuation execution at
`7161559d` in the same source-owned repository. V1 evidence remained immutable.

```text
Experiment ID: VC1-E002 / T4-v1.1
Question: Can the documented withdrawal -> fresh Submission path complete the real Lean lifecycle while preserving the failed attempt and keeping Verification non-authorizing?
Hypothesis: Existing general APIs suffice without Lean-specific Core changes.
Treatment: Authenticated withdrawal of the failed pending Proposal; fresh corrected add_claim; one corrected Lean execution; passing Verification; one authorized accept Decision; downstream start.
Controls: Immutable v1 commit/root/evidence; exact producer-key continuity; frozen source/toolchain/binary/artifact hashes; before/after Standing audits; clean-clone replay.
Task/data: Same frozen Erdos154.erdos_154_sumset fixture and source environment as VC1-E001.
Model: None; no theorem search or capability evaluation.
Tools: Vela 0.977.4; Lean 4.29.1; Mathlib 5e932f97dd25535344f80f9dd8da3aab83df0fe6.
Information available: V1 retained failure, corrected published proof fixture, documented pending-Proposal withdrawal semantics.
Resource budget: One withdrawal, one corrected Submission, one corrected Lean execution, one passing Verification, one accept Decision, one pending downstream Submission.
Primary outcome: Full Submission -> Verification -> Decision -> Event -> Standing lifecycle with failed history retained and exact replay.
Secondary outcomes: Allowed-axiom audit; producer withdrawal authority; downstream continuation; dependency-write limitation.
Inclusion criteria: Exact v1 preservation and all frozen input hashes.
Exclusion criteria: Any fail-closed no-write command would be preserved; none occurred in v1.1.
Stopping rule: Stop on hash mismatch, unexpected Lean result, disallowed axiom, stale Decision root, replay mismatch, or required Core change.
Interpretation table: Complete with no Core change = Level-1 vertical qualification; Verification changes Standing = fatal invariant failure; replay mismatch = failure; downstream only source-bound = qualify limitation.
Artifacts to preserve: V1.1 freeze, lineage/withdrawal receipts, Lean streams/status/axioms, Verification, Inbox/Decision/Events/Standing, replay, downstream task and receipt.
Frozen commit and hashes: v1.1 freeze 7161559d; v1 predecessor d4c88ceb64738f29b804a4dc7e735272bd873dd5; target SHA-256 9ac3fc83bbeba2df4739b5f3d69130876d99ea09c47d0c30977339904d74f457.
Execution authority: Supervisor RETURN TO WORKER; T4 task 01a03d3f-a006-7c51-85cd-b3392703f581; one source-owned authorized Decision.
```

Terminal source-owned commit/tree: `05b6e36fb46b840eeac533658faf6f71ad99dc06` /
`4b491446071efc4d6cd306397fa33e8b008e2f29`. Terminal Vela root:
`sha256:1f18d90faec38dfb602d1f6bfa51c0f7eb69373698baeb4e8f73cbf5dba5c82c`.
Supervisor replay reproduced all counts and the exact root from a fresh clone.

## VC1-E003 — T6 bounded R/E/V cumulative-handoff screen

The authoritative pre-outcome design freeze is source-owned at commit
`32fbe541da65b3ffa6e73d4d77d866b34aa75eca`. S0 authorizes Stage 0 only.

```text
Experiment ID: VC1-E003 / T6-C1
Question: Does native governed Standing reduce the cost or error of continuing from the accepted Erdos154 sumset theorem, compared with a competent ordinary checkout and a neutral information-equivalent export?
Hypothesis: V and E may reduce reconstruction mistakes relative to R; V may additionally reduce state-navigation mistakes relative to E. No directional proof-ability claim is made.
Treatment: V = R plus the exact terminal T4 Repository, native read-only status/why/replay, retained failure/withdrawal history, Verification/Decision provenance, uncertainty, receipts, and pending work.
Controls: Exact runtime identity, OAuth/account, prompt, tools, sandbox, Lean/evaluator, task, caps, fresh contexts, package-only retrieval, V/E scientific parity with competent R, and target/reference isolation.
Task/data: t6-erdos154-finite-uniformity-v1; exact statement in apparatus/TARGET_STATEMENT.txt; source theorem Erdos154.erdos_154_sumset.
Model: BLOCKED until exact CLI, advertised model, backend/model identity, reasoning/config, tokenizer, and account fingerprint are captured identically across arms.
Tools: Identical local shell/file tools, read-only arm package, empty writable result, and pinned instrumented Lean evaluator; no network or other paths.
Information available: R = full exact ordinary Lean checkout and documentation; V = R plus native governed state; E = R plus all V scientific fields and exact evidence bytes in a neutral export without Vela commands.
Resource budget: Per scheduled Stage-1 cell: 20 minutes, 24,000 total model tokens, 60 tool calls, 20 read/search calls, 12 participant Lean calls, plus one runner-owned final evaluator. Stage 1 is not yet authorized.
Primary outcome: Binary exact-target solve under pinned Lean plus axiom and leakage policy.
Secondary outcomes: Tokens/time/reads/searches/tool/Lean calls, duplicate investigations, invalid state assumptions, provenance mistakes, repeated verifier failures, first valid contribution, verified progress, malformed/timeout state.
Inclusion criteria: Every frozen hash and all twelve controls pass; fresh context; complete telemetry; one isolated arm; containment intact.
Exclusion criteria: Runtime/config mismatch, package/task mismatch, missing telemetry, retrieval escape, evaluator/reference leakage, runner failure, or target-proof exposure; all attempts remain preserved.
Stopping rule: No retry. Stop on any control failure, systematic R incompetence, or control-defeating anomaly. Stage 1, if later authorized, stops after exactly R -> E -> V once each.
Interpretation table: V≈E>R = structured-compilation value; V>E>R = native-interface value; V≈E≈R = no demonstrated advantage; R>V = overcompression/friction; E>V = native-interface friction. One-cell ordering is descriptive only.
Artifacts to preserve: Design and package roots; V/E equivalence; runtime capture; raw streams and trees; evaluator evidence; costs; invalid/exclusion records; blinded adjudication; every attempted cell.
Frozen commit and hashes: T6 design 32fbe541da65b3ffa6e73d4d77d866b34aa75eca; T4 commit/tree 05b6e36fb46b840eeac533658faf6f71ad99dc06 / 4b491446071efc4d6cd306397fa33e8b008e2f29; T4 Vela root sha256:1f18d90faec38dfb602d1f6bfa51c0f7eb69373698baeb4e8f73cbf5dba5c82c; Lean source commit/tree 06d1322e62aa28b860da1ec66465d913c1902c78 / 572395b76976c0b6940cbc58c15512adbc36a328; remaining hashes in T6_TASK_MANIFEST.json.
Execution authority: S0 authorizes Stage 0 only. Stage 1 requires hidden evaluator-authorability PASS, apparatus qualification, a committed receipt, and a new explicit S0 authorization.
```

Pre-outcome exclusions: the original modulus-two task and candidates A/B. They
started no model or Lean process, consumed no seed/context, and enter no
scientific denominator.

### VC1-E003 v1 Stage-0 terminal addendum

The exact v1 target failed hidden evaluator authorability and independent
apparatus reproduction before any participant or scientific cell. Both paths
identified the same parse error at the same byte-stable target. The remaining
apparatus qualified deterministic R/E/V packaging, V/E scientific-field and
blob equivalence, native V status/why/replay, exact Vela-binary custody,
containment, sealing, and fail-closed tamper checks, while correctly retaining
the unavailable exact tokenizer as a blocker.

```text
v1 disposition: EXPERIMENTAL_APPARATUS_FAILURE
v1 target SHA-256: c415ff6e7e0d592d80ed28072276948f2ae61ad7906cb3ba69b61b8ec57ce3a5
participant/model starts: 0
scientific cells: 0
scientific denominator: 0
scientific conclusion: NONE
campaign anomaly: NONE
```

VC1-D014 authorizes a strict-descendant v1.1 correction. It is not a retry of
an observed participant outcome. Only the malformed binder spelling and the
disclosed R Git-metadata packaging contradiction may change. All scientific
controls remain frozen, and Stage 1 requires a new explicit S0 authorization.

### VC1-E003 v1.1 Stage-0 preregistration

The additive v1.1 design is frozen at commit/tree
`169fd94724048c7d30d780ccceb3f3c7b38b18eb` /
`6a6da4dac8a0085bd5bbf5fbbd9c9d0d98cd7120`. Its corrected target SHA-256 is
`aa18f3bb86b194c4ec1b481a803eb11b183f3c0f3ccefbd1e971695a19b225fb`.
Every root v1 design artifact remains byte-identical. The only scientific-text
change is the semantically identical Lean bounded-sum binder spelling; the only
other contract correction makes ordinary reachable Git metadata/history an
explicit common input to R, E, and V.

VC1-D016 authorizes two independent Stage-0 outputs: a hidden reference-proof
authorability receipt and a deterministic apparatus/runtime/package receipt.
Neither task may start a model, participant context, or scientific cell. A
proposition parse PASS alone is insufficient. Stage 0 passes only if the exact
v1.1 declaration is authorable under the pinned evaluator, every package and
custody control passes, exact tokenizer identity is known, and the frozen <=5%
primary-surface token gate is evaluated. Stage 1 remains separately blocked.

## VC1-E004 — T5 Alzheimer governed literature transition

This central preregistration is frozen before lifecycle execution. The future
source-owned repository must mirror it before any Vela object is created.

```text
Experiment ID: VC1-E004 / T5-v1.1-lifecycle
Question: Can current general Vela semantics govern a real, qualified Alzheimer literature transition and leave a clean successor enough explicit state to continue without reconstructing the reviewed corpus?
Hypothesis: The exact reviewed literature-report Claim can move from unresolved frontier Standing to accepted qualified Standing through independent Verification and an authorized Decision, then replay and support one bounded pending continuation without domain-specific Core changes.
Treatment: A source-owned Vela Repository containing an initially accepted unresolved-frontier Claim, immutable S1-S6 artifacts and qualifications, a new exact Claim that supersedes the open frontier, independent review/Verification, one authorized Decision, append-only Events, terminal Standing, replay, and a clean downstream continuation.
Controls: Exact v1/v1.1 planning and review commits/hashes; no source additions; exact Claim bytes; source-family dependence; all four S2 rows; separate A/T model roles; nominal/post hoc/no-correction labels; Preis inconsistency and QAlb n=33; Edwards no-correction/missingness; author/reviewer/decider role separation; before/after Standing roots; source-owned receipts; no Core change.
Task/data: Frozen six-source CSF sPDGFRbeta/APOE4/A-T literature corpus and exact reviewer-authorized Claim in T5_V1_1_CLAIM_BOUNDARY.md.
Model: No model generates or adjudicates scientific content. A later fresh Codex task may perform only the frozen clean-machine comprehension/continuation test under a separately sealed prompt and budget.
Tools: Exact Vela 0.977.4 binary SHA-256 2db9b9bd5fef7680b208a070604c6c1f1086a2b124d4207279fcf198cc3dc858; Git; deterministic JSON/hash validators; frozen source artifacts and review receipts.
Information available: Lifecycle author receives the passed planning/review packets and source artifacts. Independent verifier receives committed Submission/evidence plus frozen primary-source representations. Clean successor receives only the committed source-owned Repository, documented Vela surfaces, and a frozen comprehension/continuation task—no prior conversation.
Resource budget: One initial-frontier lifecycle, one reviewed transition lifecycle, one authorized updating Decision, one replay, one fresh clean-successor task capped at 15 minutes, 15,000 total tokens where observable, 40 tool calls, and no network/source additions.
Primary outcome: Exact Submission -> independent Verification -> authorized Decision -> Event -> Standing transition, clean replay root equality, and 10/10 correct comprehension answers plus one useful bounded pending continuation that does not broaden scientific Standing.
Secondary outcomes: Preservation of rejected/unsupported propositions; source/evidence-class errors; provenance/authority mistakes; reads/tool calls/time to comprehension and pending continuation; Core/schema pressure; clean-machine replay and receipt resolution.
Inclusion criteria: Every frozen artifact hash passes; v1 remains immutable; Claim bytes and qualifications are exact; reviewer and Decision authority are distinct; all Vela objects are source-owned and replayable; clean successor has no prior task context.
Exclusion criteria: Source drift/addition, broader or causal/medical Claim, missing advisory or denominator, conflated evidence class, unbound authority, replay mismatch, hidden mutable state, domain-specific Core change, clean-agent retrieval escape, or incomplete telemetry; preserve every invalid attempt.
Stopping rule: Stop on any hash mismatch, overclaim, unavailable decisive evidence, role/authority collapse, Verification changing Standing, stale Decision root, replay mismatch, required Core change, or clean-successor control failure. No claim repair after outcome; return to S0.
Interpretation table: Full lifecycle plus 10/10 clean continuation = Level-1 real-science vertical qualification; lifecycle passes but continuation fails = governance qualification with handoff failure; lifecycle needs biology-specific semantics = abstraction-boundary failure; replay/authority invariant fails = protocol failure; any broader scientific inference = invalid experiment.
Artifacts to preserve: Frozen preregistration; source packets; initial and updated Claims/Standing; all Submissions/Verifications/Decisions/Events; exact evidence and qualifications; receipts; roots; replay logs; clean-successor sealed task/raw transcript/scoring; invalid/refusal evidence; final report.
Frozen commit and hashes: Planning 337f8bd836b06474fbbf00be6275b673a950ff7a / f8eb8b1c46ee07f6048098f43019e65bee64d9fb; review 427e234c77ebea6022ca835c0ee236e2c1ab0110 / 38cfa8925c123f27ad002cd23d4e483584d46060; review receipt SHA-256 65fa82e8c8239b0303b092856b572dba37a7d39b6ec0b0bf678129b8d9c21e9f; remaining hashes in the two receipts.
Execution authority: VC1-D013. Source-owned lifecycle execution only; clean-successor task requires a post-lifecycle S0 audit and separate launch.
```

No biological effect is being newly estimated. The scientific transition is
the governed acceptance of an exact literature-report statement with explicit
unestablished propositions and limitations.

### VC1-E004 lifecycle checkpoint and successor launch authority

The source-owned lifecycle passed at commit/tree
`363d1210e33f951739b6281097054f179ee04123` /
`ba7c3381899c76ae972a71ed7355c3e1fcfc087c`, with terminal Repository root
`sha256:785fa897ac8ffa9e8dd92756090923ee9e8ce3ec593ed07bb73baa63aa58a79a`.
The public lifecycle receipt SHA-256 is
`1bc4b26523b28b4437ae2b0e47e4cbc79aae3bcf3f9719d8afc9c493aa30a1b9`.
The clean-successor participant package remains exactly the pre-outcome freeze
bound by package receipt SHA-256
`acc892381d14dd529aae248dea9d7c7a18a642b3ae3cac7dcdd49e42dc16fd28`.
Its answer key and scoring contract remain outside the participant package at
separate commit/tree `affe34ba5933865f1a8705e40285131a29ec7b33` /
`687b1a1c56f663fe7f721a167d18e10b90edace4`.

VC1-D015 authorizes exactly one fresh blind successor. This is the final
pre-outcome launch authorization; its 10/10 comprehension and one-pending-
Proposal gate may not be amended after the participant context opens.

### VC1-E004 clean-successor terminal addendum

The single authorized successor attempt is terminal at commit/tree
`f16d4ef929e16d5ee2510c77d621b9b234d29e76` /
`b08e8143116e797fe80bd810acfe907b9e2b43d6`. Scientific tool call four emitted
approximately 18,789 observable stdout tokens before transport truncation,
crossing the 15,000-token cap. The participant stopped immediately and did not
execute `vela submit`.

```text
attempt validity: VALID_BOUNDED_ATTEMPT
clean-successor gate: FAIL
failure reasons: OBSERVABLE_TOKEN_BUDGET_OVERRUN; ZERO_PENDING_CONTINUATION_PROPOSALS
new Proposal / Verification / Decision / Event: 0 / 0 / 0 / 0
terminal Repository root: sha256:785fa897ac8ffa9e8dd92756090923ee9e8ce3ec593ed07bb73baa63aa58a79a
retry: FORBIDDEN
campaign anomaly: NONE
```

The lifecycle arm remains PASS; the full `lifecycle + blind continuation`
interpretation is FAIL. No post-outcome screen or altered successor protocol is
authorized.

### VC1-E003 v1.1 Stage-0 terminal addendum

The two independently authorized v1.1 Stage-0 outputs are terminal. The hidden
evaluator mechanically applied only the authorized binder correction to the
sealed v1 candidate. The proof body remained byte-identical, but the corrected
candidate failed Lean at line 27:17. No proof repair, search, inspection, or
rerun occurred. The public apparatus independently qualified every non-token
component, but could not obtain an exact tokenizer identity/version for the
frozen `gpt-5.6-sol` runtime; therefore the exact token counts and <=5% gate
remain null.

```text
hidden-evaluator result: HIDDEN_EVALUATOR_AUTHORABILITY_FAIL
hidden-evaluator commit/tree: 75f3634443e69667b01768e3f195860df163f94f / f50adc3a2e3342abd11ec9bf3a5f4e52a9f92851
authorability receipt SHA-256: 348d904b41ef34fb2046f2a91d58b2ac6f49023cd9ba512f8be9621b20676084
corrected candidate SHA-256: 26c4e002749944d2ea2909abf2f234fb7c9379d5f9612b5df490eb4b44267823
proof-body SHA-256: 4d3f15525fe354aca5aa61533d5049a21456c53440f291d39b67721e1fe41578
diagnostic stream SHA-256: 8e2d260178e5603601df81fb34c62c5093db3fd833bdf50ac8fee26a879cb4eb
apparatus result: BLOCKED_RUNTIME_IDENTITY
apparatus commit/tree: a835bcc23b468e9588f170a13db17cafd8b10b5d / 97c3ad0b2760e8b34a50e86227ab920203203cf4
apparatus receipt root: sha256:f774f8c4002e381bb5b47c9bedd5da47f50d98fe984515106d6baa0b3e449625
participant/model starts: 0
scientific cells: 0
scientific denominator: 0
Stage 1: CLOSED WITHOUT EXECUTION
scientific conclusion: NONE
campaign anomaly: NONE
```

The experiment establishes no R/E/V comparison and no cumulative-handoff
effect. VC1-D018 closes candidate C and forbids rescue, replacement, or proxy
tokenization. The qualified apparatus components remain reusable evidence of
instrument construction only.
