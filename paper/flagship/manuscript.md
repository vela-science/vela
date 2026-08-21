---
title: "Replayable scientific state under local authority"
subtitle: "Vela's executable substrate, correction evidence, and open inheritance test"
author: "Vela contributors"
date: "Working draft, 2026-08-21"
lang: en
---

# Abstract

Scientific repositories preserve files and commits but often leave four facts
implicit: the exact claim under review, the scope of a check, the authority
that admitted a change, and the downstream consequences of a correction. Vela
defines a Git-custodied protocol for those facts. Protocol 1 retains canonical
Claims, authenticated Submissions, scoped Verification Records, local
Decisions, append-only Events, and replayed Standing. Derived projections and
external controllers remain outside the authority boundary.

This paper states a layered claim and tests each layer separately. Executable
conformance supports deterministic replay for the fixed Protocol 1 waist. A
portable-divergence reference imports the same authenticated Submission into
two Repositories whose distinct local authorities accept and reject it, then
replays both histories to different Standing. A retained map-to-target trace
shows an external controller orienting work while a separate attributed
Decision supplies the only Standing-changing operation. One real Erdős 264
source correction binds five direct consumers, a scoped proof repair, replay,
and a cold-successor handoff.

The empirical inheritance claim remains open. A preregistered internal
16-session comparison produced 130 points and five exact successes in the Vela
arm versus 112 points and zero exact successes in the Git/documents arm, with a
restricted-time ratio of 0.388463730674583. The Vela arm also recorded three
authority errors and the Git/documents arm eight. The preregistered positive
gate was `not_supported`. A byte-bound audit found exact correction pairs,
consequence classifications, and safe actions in all 16 responses; the misses
arose from prose-versus-code authority fields and path-without-digest source
bindings. Those findings motivate a fresh protected 36-cell, three-family,
three-arm held-out test. They do not rescore the sealed trial or establish
lift.

# 1. Claim

Vela's central claim has four layers.

**L1, replay.** Given the same Protocol 1 canonical bytes, complete valid Git
history, and trust inputs, conforming readers derive the same object roots,
Event history, Repository root, and Standing.

**L2, local authority.** An authenticated Submission can cross Repository
boundaries without carrying Decision authority. Each Repository applies its
own authorization and scientific judgment, so two valid histories may reach
different Standing.

**L3, removable control.** A controller may read projections, select work, and
prepare a Submission. It cannot admit an Event. Removing the controller leaves
canonical history and strict replay intact.

**L4, inheritance value.** A Vela-organized correction packet may lower the
cost of finding affected claims and choosing safe next actions relative to Git
and documents that contain the same information.

The first three layers describe the executable substrate and its bounded
reference traces. The fourth is an empirical hypothesis. The sealed
16-session study did not satisfy its registered positive gate, so this draft
does not claim inherited-correction lift.

# 2. Formal model

This section introduces analysis notation for Protocol 1. It adds no protocol
object or Core semantic.

Let $B(o)$ denote the RFC 8785 canonical bytes of object $o$, and let
$H(o)=\mathrm{SHA256}(B(o))$. A Repository $R$ retains a bounded object set,
an append-only authority history, and ordinary Git identity. A valid Decision
$D$ must pass schema, root, authentication, authorization, read-set, and
current-head checks under the local authority state $A_R$.

Define the admitted transition relation

$$
T_R(S_i,A_i,D_i)=(S_{i+1},A_{i+1})
$$

only when $D_i$ passes those local checks. A Submission, Verification Record,
Git commit, or controller output cannot instantiate this relation. Strict
replay folds the admitted Events in canonical history order:

$$
\operatorname{Replay}(E_0,\ldots,E_n)=S_n.
$$

Protocol 1's determinism claim concerns conforming readers over identical
valid inputs. It does not say that local authorities will make the same
Decision or that accepted Claims represent global truth.

A portable Submission $P$ binds producer identity, exact Artifacts, and a
proposed bounded transition. It binds no Repository. Two Repositories may
therefore import the same $P$ and derive the same Claim identity while their
local functions $A_{R_1}$ and $A_{R_2}$ admit different Decisions. The
result $S_1 \ne S_2$ demonstrates interoperability without consensus.

A projection $\pi(S,H(R))$ reads root-bound state and carries no authority. An
external controller $C$ may compute an obligation from $\pi$, run a native
tool, and produce a Submission. The protocol boundary remains:

$$
C(\pi(S,H(R))) \rightarrow P \rightarrow V \rightarrow D \rightarrow E
\rightarrow \operatorname{Replay}(E).
$$

The Decision term is the only Standing-changing term in this expression.

For a correction from predecessor Claim $c_0$ to successor $c_1$, a bounded
dependency profile classifies represented consequences as affected,
unaffected, must-reassess, or presently unprovable. The classification states
what follows within declared source and relation bounds. It does not claim
coverage of unknown source material.

# 3. Falsification criteria

The program uses separate failure criteria for substrate and empirical claims.

1. **Canonical replay failure.** Two conforming readers accept the same valid
   canonical history and trust inputs but derive different roots or Standing.
2. **Authority-containment failure.** A Submission, Verification, projection,
   foreign Decision, or controller output changes local Standing without an
   authorized local Decision.
3. **Portability failure.** Two Repositories cannot retain the same valid
   Submission bytes and derived Claim identity without importing each other's
   authority.
4. **Removability failure.** Strict replay depends on the controller, hosted
   projection, model session, or workflow that produced the evidence.
5. **Correction-bound failure.** A claimed consequence class lacks its exact
   source, relation, or completeness bound.
6. **Inheritance failure.** A preregistered matched comparison misses its
   success, authority-safety, or time gate.
7. **Reproduction failure.** The exact artifact package does not regenerate
   registered roots and categorical results from its bound commit.

The sealed 16-session experiment triggered failure criterion 6: its positive
gate failed. The result remains part of the evidence rather than disappearing
from the denominator.

# 4. Methods

## 4.1 Protocol and replay

Protocol 1 fixes canonical JSON, SHA-256 identities, DSSE signatures, closed
JSON schemas, the Submission-to-Standing lifecycle, and complete Git history.
Rust, Python, and JavaScript conformance readers check the normative vectors.
Passing those checks establishes implementation agreement over the selected
surface. It supplies no scientific Decision.

## 4.2 Portable divergence

The reference fixture imports Submission
`sha256:f1669cdfa498ff85c162bce6173f04b39cdf7620fb198a19b45f6d932302204a`
into two disposable Repositories. Test-support supplies distinct authenticated
device-and-UID principals while leaving production identity resolution
unchanged. One Repository records a local Verification and accepts; the other
rejects without importing that Verification. Retained Git bundles bind every
sequence-one authority record, Decision, Event, event-log, Repository,
projection, and Standing root.

## 4.3 Controller/substrate trace

The map-to-target artifact freezes a derived map and the first bounded target,
then records a native producer run and scoped Verification. Accepted-event
delta remains zero until a human performer makes a separate attributed
Decision. A key-free materializer checks the terminal Decision, performs a
clean-clone replay, and regenerates the read-only map. The trace exposes one
product defect: the first target packet remained stale after the Decision.
Later source-local closure repaired the controller's target progression
without changing Protocol 1.

## 4.4 Real correction case

The Erdős 264 source transition changes bounded perturbations from natural to
integer values. The evidence binds the before and after Formal Conjectures
commits, full-index diff, source blobs, and five direct theorem consumers. A
separate proof-repair episode retains a source-preserving Lean artifact,
scoped Verifications, an attributed accepting Decision, strict clean-clone
replay, and a context-free successor handoff.

The preregistered matched repair study produced zero exact native passes in
both arms. The later unlimited-heartbeat repair belongs to a separate
engineering episode and does not rescore that result.

## 4.5 Sealed inherited-correction comparison

The synthetic fixture contains one upstream calibration correction and a
bounded dependency chain with all four consequence classes. The two conditions
contain the same 77 candidate-visible atomic facts and the same six exact
source and evidence files. Git/documents presents ordinary history, claims,
dependencies, and a manifest. Vela presents correction, replay, per-Claim
bindings, and `why` records. Protected adjudication stayed outside participant
containers.

The runtime assigned 16 fresh participant instances, eight per condition, to
one-turn `gpt-5.6-sol` sessions at high reasoning effort. Each cell had one
single-use permit, 600 seconds, no tools, no continuation, no retry, and no
substitution. The runner froze all terminal receipts, consumed permits, event
streams, responses, and packet roots before it opened scoring. Independent
review reproduced custody and canonical result serialization across CPython
3.10 through 3.14.

## 4.6 Post-result audit

The audit read the frozen responses and adjudication without rerunning or
rescoring a cell. It compared predecessor and successor identity, four
consequence classes, four safe-action codes, Standing-effect fields, and source
bindings. Independent review recomputed every row and the audit root.

## 4.7 Held-out replication, work in progress

The prospective design uses three unseen correction families with different
vocabularies, dependency topologies, and authority regimes. The registration
must fix each regime and its safe-action boundary before packet generation.
Each family contains the four bounded consequence classes.

The three arms separate presentation structure from Vela-specific governance
and inheritance:

- **Git/documents, $G$.** Ordinary source, history, dependency, and evidence
  documents present the candidate-visible facts.
- **Neutral structured-state wrapper, $N$.** A typed current/superseded state
  and dependency view presents the same facts. It contains no Vela Repository,
  Decision, Event, Standing, or authority replay.
- **Vela, $V$.** Correction, replay, per-Claim bindings, and authority-scoped
  views present the same facts through the existing protocol substrate.

All three arms receive identical atomic facts. The implementation must match
packet and prompt length prospectively under a frozen comparison rule, before
participants or the implementation lane can inspect protected adjudication.
Four fresh participant instances per arm and family yield
$3 \times 3 \times 4 = 36$ fixed cells, balanced at 12 per arm. The response
contract uses a closed generic authority code and structured `{path, sha256}`
source bindings. Explanatory prose remains outside exact scoring.

For each preregistered primary metric $M$, the analysis reports additive arm
differences:

$$
\Delta_{\mathrm{structure}} = M_N - M_G,
\qquad
\Delta_{\mathrm{governance}} = M_V - M_N,
$$

and

$$
\Delta_{\mathrm{total}} = M_V - M_G
= \Delta_{\mathrm{structure}} + \Delta_{\mathrm{governance}}.
$$

The registration must fix the favorable direction for error or loss metrics.
Restricted-time ratios remain secondary descriptive measures and cannot move a
primary additive gate. The length- and cost-matched wrapper comparison in
[StateMem v1](https://arxiv.org/abs/2608.19652v1) motivates this identification
strategy: a neutral structured view helps estimate the effect of state
structure without assigning that wrapper Vela semantics. StateMem's reported
results do not count as evidence for Vela, this benchmark, or adoption.

An independent evaluator must custody the protected labels and action answers,
publish only their root before execution, and release them after all captures
freeze. The implementation must bind a fresh seed, 36-cell schedule, packets,
information-equivalence and length-matching proofs, canonical decimal metrics,
single-use permits, and fail-closed adversaries before any model call. The
freeze must place every permit in `hold`, and prelaunch review must PASS before
one may be consumed. The protocol allows no retry or substitution. This
manuscript revision contains no frozen registration, permit set, independent
prelaunch verdict, participant call, or result for this design. Its status is
`not_run`.

# 5. Results

| Evidence layer | Exact result | Claim status |
| --- | --- | --- |
| Protocol 1 | Conformance root `sha256:6ca327d6ec6f56c051e236be9eee42629e1f67dce393c1518e051ff8c14b279e`; independent root readers agree on the reviewed surface. | Bounded implementation agreement |
| Portable divergence | Same Submission and Claim roots; distinct local principals, Decisions, Repository roots, and accepted versus unassessed Standing; clean-clone replay passes. | Bounded interoperability without consensus |
| Controller trace | Map, native work, Verification, separate Decision, replay, and remap retained; controller authority effect `none`. | One first-party removability trace |
| Erdős 264 | One real source correction, five direct consumers, source-preserving repair, attributed Decision, replay, and successor handoff; matched study 0/1 versus 0/1. | Action-complete case, no causal lift |
| Inherited correction | Git/documents: 112 points, 0 exact, 8 authority errors, restricted mean 600 s. Vela: 130 points, 5 exact, 3 authority errors, restricted mean 233.07823840475 s. Ratio 0.388463730674583. | `positive_gate=not_supported` |
| Miss audit | All 16 pair, class, and action answers exact; 11 semantic-none prose misses; 8 path-without-digest misses. | Directional contract and navigation evidence |
| Held-out replication | Proposed 36-cell, three-family, three-arm design; no frozen registration or executed result. | `not_run`; open |

The 16-session counts and time ratio show a directional internal difference.
The registered gate required at least six Vela exact successes, no Vela
authority errors, no exact-success disadvantage, and a ratio at most 0.8. The
Vela arm failed the first two requirements. The result therefore cannot support
a positive lift claim.

The audit narrows the prospective repair. Every participant selected the exact
correction pair, consequence classes, and safe actions. Eleven participants
described a null Standing effect in prose instead of returning the scorer's
literal `none` token. All eight Git/documents participants cited accurate paths
but omitted a digest from a free-text field. The frozen scorer treated those
contract misses as authority or exactness failures and mapped failed exactness
to 600 seconds. The registered result keeps that treatment. A new study can
test a closed contract without changing the old score.

# 6. Artifact manifest

| Artifact | Commit or root | Contents | Reproduction status |
| --- | --- | --- | --- |
| Current Protocol 1 and portable divergence | commit `4685462c44b1f073870f31025ae73d1d8770ce73`, tree `13c5e0cf2e64be907cee4c0fd740ab0027118e13` | Normative protocol, conformance vectors, two complete divergence histories | In current main; one-command check |
| Portable expected vector | `sha256:858019d298f55295fe92989bb23a343ce73b6976338f36c7c637c82272274041` | Exact principals, Decisions, Events, Repository roots, projections, and Standing | In current main; one-command check |
| Controller trace | pre-run `sha256:e0a517d543ce448917f6baa1a620727431caa53b0590f247bbf3fe9f5c3ed6d6`; post-verification-map `sha256:439a804908890e4029922cc91cdd0a79122187d573530fc760a419d90786be21`; post-decision `sha256:b29e8cbb50aff3cc81a4ac6f4cf261b9a3ca9d80dbe69614d9a771116d80151c` | Root-bound map, target, Verification, Decision, replay, and remap | In current main; source-only integrity check |
| Erdős 264 case | commit `b6e554513346f515090e013a3484548261b7b93d`, tree `ab3df803bf11abc9adc4915be8be573501f454dd`; result root `sha256:f9c009ec0e53cfd0362b924b440ba44cee243af5248906da1c82f516ec4c7585` | Real source transition, matched null result, post-study repair, Decision, replay, handoff | Historical retained bytes; external source checkout needed for full source revalidation |
| Sealed 16-session result | producer `7641d775911f6026a9c36649d6cf1354dd1f70c0`; result `sha256:48c3ab674e1ef707a207c2a5cf8addab16d7209e8229def76f0f1568a466f83f` | Registration, packets, 16 captures, scorer, canonical result | Independently reviewed at `1f7ebabee72058619e8081d71c3fc4325b81f64b` |
| Capture and custody | capture `sha256:0e5f60fa1dc78e531d44cb8fff626e73c6b2c0017bbcec52e41220cbfac686fd`; custody `sha256:619512f17009dd92c651a687cbc17dd5899c0b908619d82de465b9747a7aa3f5` | Exact denominator, permits, terminal receipts, provider events, and responses | Independently reconstructed |
| Miss audit | producer `de13073ff8f3a9f2958f8c93c848205c533ddb1e`; artifact `sha256:8463024ee31116c33cee9e43262286bb78855654ecc974e77818bf4dfac581af` | Frozen per-cell classifications and prospective fixes | Independently reviewed at `720053e9fc0cb95d2b2258516663300f43b29c16` |
| Held-out three-family design | root pending | Prospective protected 36-cell registration, three matched arms, packets, answer-key commitment, assignments, and capture plan | Work in progress; `not_run`; unreviewed |

# 7. Reproduction contract

The paper's current source tree provides one entry point:

```bash
./paper/flagship/reproduce.sh
```

The command must fail closed unless it can resolve every bound commit and
reproduce the listed roots. It runs current Protocol 1 conformance, the
portable-divergence test, inherited-correction verification and deterministic
serialization checks, audit-manifest reconstruction, and retained Erdős 264
unit checks in disposable detached worktrees. It prints no protected held-out
labels and invokes no provider or authority action.

The final public bundle must include every tracked source member plus the exact
external Formal Conjectures and Erdős source members needed for the real
correction check. A bundle qualifies as paper-ready after an independent task
reconstructs it from the manuscript commit and reproduces the manifest. An
outside group running the same bundle after publication would add external
reproduction; the current claim set does not require or presume that event.

# 8. Limitations

The empirical corpus contains one synthetic correction family and one bounded
real mathematical correction. The 16-session trial used one model family,
first-party infrastructure, and eight sessions per condition. It failed its
positive gate. The audit found output-contract and navigation effects within
that fixture; it cannot separate every model, packet, or scorer effect.

The three-arm design may separate structured presentation from Vela-specific
governance only within its frozen families and authority regimes. Prospective
length matching cannot make distinct representations identical, and the
neutral wrapper cannot test Repository authorization or Standing replay. The
design remains unreviewed and `not_run`.

The portable-divergence identities are synthetic test-support principals. The
controller trace and Erdős 264 handoff used first-party operators on colocated
machines. The real correction case has five direct consumers, but its relation
coverage cannot stand in for other fields or repositories. Passing a Lean
check establishes the exact scoped formal observation rather than informal
importance or truth beyond the statement.

Protocol 1 remains a release candidate. Conformance demonstrates agreement
between implementations over a fixed selection. It does not establish
adoption, resistance to every malicious implementation, or the quality of a
local scientific Decision.

# 9. No-go claims

This paper does not claim global truth, global consensus, transported Standing,
automatic scientific acceptance, or controller authority. It does not call a
Verification a Decision. It does not report the 16-session time ratio as
positive lift, because the registered gate failed. It does not count the
post-result audit as a rescore. It does not call the held-out design executed,
reviewed, or successful. It assigns no Repository, Decision, Event, Standing,
or authority semantics to the neutral wrapper. It does not use StateMem's
results as Vela evidence. It does not claim external reproduction, adoption,
or general productivity.

# 10. Current paper-ready gate

The internal paper-ready gate consists of:

1. exact Protocol 1 and portable-divergence evidence;
2. a bounded controller/substrate trace and real correction case;
3. the sealed negative 16-session result and reviewed miss audit;
4. a fresh, protected held-out registration followed by fixed-denominator
   execution and independent exact-byte method review; and
5. a public-ready source and evidence bundle that passes the one-command
   reproduction contract.

Items 1 through 3 exist. Items 4 and 5 remain open in this working draft.
External execution after publication can test the package under a new operator
and institution. The manuscript will report that future event only after it
occurs.

## 10.1 Compressed critical path

These planning ranges assume the existing bounded scope and no infrastructure
failure. The fastest case assumes first-pass method and result review. The
expected case allows one narrow correction cycle at each review boundary.

| Stage | Fastest case | Expected duration | Exit condition |
| --- | --- | --- | --- |
| Implement and freeze the three-arm packets, wrapper, registration, and permits | 8 hours | 1--2 days | Immutable 36-cell design with all permits held |
| Independent method and exact-byte prelaunch review | 4 hours | 0.5--1 day | Exact PASS for the frozen design |
| Sequential execution and custody | 8--12 hours | 1--2 days | Exactly 36 terminal cells frozen, with failures retained and zero retries |
| Deterministic scoring and independent post-result review | 4--8 hours | 0.5--1 day | Canonical result plus exact PASS or a reported blocker |
| Manuscript and public reproduction-bundle integration | 2--4 hours | 0.5--1 day | Updated claim matrix, artifact manifest, render, and one-command check |
| **Internal paper critical path** | **26--36 hours** | **3.5--7 days** | Internal paper-ready gate evaluated without an external executor |
| External reproduction after publication | 0 hours added | 0 days added | Downstream validation, outside this paper-ready gate |
