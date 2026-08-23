---
title: "Replayable scientific state under local authority"
subtitle: "Vela's executable substrate and two negative inheritance tests"
author: "Vela contributors"
date: "Working draft, 2026-08-22"
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

Vela operates as a global registry over plural repository-local authority
histories. Derived global Frontiers query current Standing across Repositories
and carry no authority. The registry does not assert a single global truth
ledger.

This paper states a layered claim and tests each layer separately. Executable
conformance supports deterministic replay for the fixed Protocol 1 waist. A
portable-divergence reference imports the same authenticated Submission into
two Repositories whose distinct local authorities accept and reject it, then
replays both histories to different Standing. A retained map-to-target trace
shows an external controller orienting work while a separate attributed
Decision supplies the only Standing-changing operation. One real Erdős 264
source correction binds five direct consumers, a scoped proof repair, replay,
and a cold-successor handoff.

The empirical inheritance claim is unsupported. A preregistered internal
16-session comparison returned `positive_gate=not_supported`. A later held-out
benchmark separated Git/documents, a neutral structured-state wrapper, and
Vela across three synthetic correction families. Each control arm achieved
12/12 exact and impact-complete results with zero authority errors. Vela
achieved 11/12 exact and 12/12 impact-complete results with one authority
error. Restricted means were 12.800895867, 13.98268798558333, and
63.252235329 seconds for Git/documents, the wrapper, and Vela. The registered
structure, governance/inheritance, and total gates all failed. Independent
review passed the exact 36-cell custody and negative result. These experiments
support no Vela lift claim.

# 1. Claim

Vela's central claim has four layers.

**L1, replay.** Given the same Protocol 1 canonical bytes, complete valid Git
history, and trust inputs, conforming readers derive the same object roots,
Event history, Repository root, and Standing.

**L2, plural local authority.** An authenticated Submission can cross
Repository boundaries within the global registry without carrying Decision
authority. Each Repository applies its own authorization and scientific
judgment, so two valid histories may reach different Standing. Derived global
Frontiers may query both histories but cannot reconcile or change them.

**L3, removable control.** A controller may read projections, select work, and
prepare a Submission. It cannot admit an Event. Removing the controller leaves
canonical history and strict replay intact.

**L4, inheritance value.** A Vela-organized correction packet may lower the
cost of finding affected claims and choosing safe next actions relative to Git
and documents that contain the same information.

The first three layers describe the executable substrate and its bounded
reference traces. The fourth is an empirical hypothesis. Both the sealed
16-session study and the held-out 36-cell study missed their registered
positive gates, so this draft does not claim inherited-correction lift.

# 2. Formal model

This section introduces analysis notation for Protocol 1. It adds no protocol
object or Core semantic.

Let $B(o)$ denote the RFC 8785 canonical bytes of object $o$, and let
$H(o)=\mathrm{SHA256}(B(o))$. A Repository $R$ retains a bounded object set,
an append-only authority history, and ordinary Git identity. A valid Decision
$D$ must pass schema, root, authentication, authorization, read-set, and
current-head checks under the local authority state $A_R$.

The global registry indexes plural Repositories and their portable records.
A global Frontier $F$ is a derived query over current repository-local
Standing. It owns no records, supplies no authority, and cannot substitute for
any Repository's Decision boundary. Registry-wide visibility therefore does
not create a single global truth ledger.

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

Both inherited-correction experiments triggered failure criterion 6. Their
positive gates failed, and both fixed denominators remain part of the evidence.

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

## 4.7 Held-out replication

The frozen design uses three unseen correction families with different
vocabularies, dependency topologies, and repository-local authority regimes.
The registration fixes each regime and its safe-action boundary. Each family
contains the four bounded consequence classes.

The three arms separate presentation structure from Vela-specific governance
and inheritance:

- **Git/documents, $G$.** Ordinary source, history, dependency, and evidence
  documents present the candidate-visible facts.
- **Neutral structured-state wrapper, $N$.** A typed current/superseded state
  and dependency view presents the same facts. It contains no Vela Repository,
  Decision, Event, Standing, or authority replay.
- **Vela, $V$.** Correction, replay, per-Claim bindings, and authority-scoped
  views present the same facts through the existing protocol substrate.

All three arms receive identical atomic facts. The frozen comparison rule
matches packet and prompt length before participants or the implementation lane
can inspect protected adjudication.
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

The registration fixes the favorable direction for error or loss metrics.
Restricted-time ratios remain secondary descriptive measures and cannot move a
primary additive gate. The length- and cost-matched wrapper comparison in
[StateMem v1](https://arxiv.org/abs/2608.19652v1) motivates this identification
strategy: a neutral structured view helps estimate the effect of state
structure without assigning that wrapper Vela semantics. StateMem's reported
results do not count as evidence for Vela, this benchmark, or adoption.

The final successor registration bound the same three-arm design after
fail-closed runtime compatibility checks. Its registration root is
`sha256:60acdfa31d25f9df5f342b75caf8e65426c5b71fa320c36fe5568de9fbf13b10`
and its assignment root is
`sha256:64a356db4800b6fb04090ae81a6c2d33bf37ad8b71e92e01567edc5fa6362e72`.
The runner executed 36 sequential attempt-one cells, 12 per arm, with one turn
and response per cell. It recorded zero retries, substitutions, timeouts,
tools, or compactions. Each consumed permit and terminal response entered
custody before the next cell began. Two stopped predecessor registrations and
the neutral runtime calibration remained outside the denominator.

The producer sealed the complete capture at
`5694bebac03b062d6acdce5a2a900551850e6a1c`, tree
`feec0ff21b9b13be8cbb97083f441ef66bdd48f2`. The capture root is
`sha256:4a592d88b43dc02d5495d7679834535d6fa97f20759600400253677a946f87fd`
and the complete custody root is
`sha256:ccf69e70a3887c8a9f9ddffa2d62051e114a8974b2d2ae83c72366a1eb98dcef`.
One protected scoring process produced the result at
`4524c8f776943a267e04e03e9a237ecaed14bc2`, tree
`4d5650a999ac0be59e71d5bd664e885cad5192c7`. The score-capture root is
`sha256:f74229b3346cf56e2128d78b366f5fb99380872c27285d196c13862738bc8e98`
and the canonical result root is
`sha256:92eed5bcb9e6b647d52a53282563077d3829b28c426e0dd9898a073f2590b8a5`.
Independent review `e6d8348bea3a57e88c5f9426d44a480b7a026fbd`
reconstructed the custody and arithmetic without opening the protected key or
rescoring the study.

# 5. Results

**Protocol 1.** Independent readers agree on the reviewed surface at
conformance root
`sha256:6ca327d6ec6f56c051e236be9eee42629e1f67dce393c1518e051ff8c14b279e`.
This supports bounded implementation agreement.

**Portable divergence.** The same Submission and Claim roots enter histories
with distinct local principals, Decisions, Repository roots, and accepted
versus unassessed Standing. Clean-clone replay passes. This supports bounded
interoperability without consensus.

**Controller trace.** The retained trace binds the map, native work,
Verification, separate Decision, replay, and remap. The controller authority
effect is `none`. This is one first-party removability trace.

**Erdős 264.** The case binds one real source correction, five direct
consumers, a source-preserving repair, attributed Decision, replay, and
successor handoff. The matched study returned 0/1 versus 0/1, so it supports no
causal lift.

**Sealed inherited correction.** Git/documents recorded 112 points, 0 exact
successes, 8 authority errors, and a 600-second restricted mean. Vela recorded
130 points, 5 exact successes, 3 authority errors, and a
233.07823840475-second restricted mean. The time ratio was
0.388463730674583 and `positive_gate=not_supported`.

**Miss audit.** All 16 pair, class, and action answers were exact. The audit
found 11 semantic-none prose misses and 8 path-without-digest misses. This is
directional contract and navigation evidence.

**Held-out replication.** Git/documents achieved 12/12 exact and
impact-complete results, 0 authority errors, and a 12.800895867-second
restricted mean. The wrapper achieved 12/12, 12/12, 0, and
13.98268798558333 seconds. Vela achieved 11/12, 12/12, 1, and 63.252235329
seconds. Structure, governance/inheritance, and total gates are false;
`positive_gate=not_supported`.

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
to 600 seconds. The registered result keeps that treatment and remains
unchanged.

The held-out study used that prospective closed contract and added a neutral
structured-state control. Both controls achieved perfect exact and
impact-complete counts. Vela missed one exact result through an authority error
and took longer under the registered restricted-time measure. The structure,
governance/inheritance, and total gates all failed. This fixed synthetic result
does not show a Vela advantage over either control. It also cannot establish a
general Vela disadvantage beyond the three registered families, one model, and
one runtime.

# 6. Artifact manifest

**Current Protocol 1 and portable divergence.** Commit
`4685462c44b1f073870f31025ae73d1d8770ce73`, tree
`13c5e0cf2e64be907cee4c0fd740ab0027118e13`, retains the normative protocol,
conformance vectors, and two complete divergence histories. The portable
expected vector is
`sha256:858019d298f55295fe92989bb23a343ce73b6976338f36c7c637c82272274041`.

**Controller trace.** The pre-run, post-Verification-map, and post-Decision
roots are `sha256:e0a517d543ce448917f6baa1a620727431caa53b0590f247bbf3fe9f5c3ed6d6`,
`sha256:439a804908890e4029922cc91cdd0a79122187d573530fc760a419d90786be21`,
and `sha256:b29e8cbb50aff3cc81a4ac6f4cf261b9a3ca9d80dbe69614d9a771116d80151c`.

**Erdős 264 case.** Commit `b6e554513346f515090e013a3484548261b7b93d`,
tree `ab3df803bf11abc9adc4915be8be573501f454dd`, retains result root
`sha256:f9c009ec0e53cfd0362b924b440ba44cee243af5248906da1c82f516ec4c7585`.
Full source revalidation also needs the bound external source checkout.

**Sealed 16-session result and audit.** Producer
`7641d775911f6026a9c36649d6cf1354dd1f70c0` retains result
`sha256:48c3ab674e1ef707a207c2a5cf8addab16d7209e8229def76f0f1568a466f83f`.
Capture and custody roots are
`sha256:0e5f60fa1dc78e531d44cb8fff626e73c6b2c0017bbcec52e41220cbfac686fd`
and `sha256:619512f17009dd92c651a687cbc17dd5899c0b908619d82de465b9747a7aa3f5`.
Review `1f7ebabee72058619e8081d71c3fc4325b81f64b` passed the result. Audit
producer `de13073ff8f3a9f2958f8c93c848205c533ddb1e` retains artifact
`sha256:8463024ee31116c33cee9e43262286bb78855654ecc974e77818bf4dfac581af`,
reviewed at `720053e9fc0cb95d2b2258516663300f43b29c16`.

**Held-out sealed capture.** Commit
`5694bebac03b062d6acdce5a2a900551850e6a1c`, tree
`feec0ff21b9b13be8cbb97083f441ef66bdd48f2`, retains capture
`sha256:4a592d88b43dc02d5495d7679834535d6fa97f20759600400253677a946f87fd`
and custody
`sha256:ccf69e70a3887c8a9f9ddffa2d62051e114a8974b2d2ae83c72366a1eb98dcef`.
Pre-score review `b634523ea1c85dce697404968cf7492f09a6412f` passed those bytes.

**Held-out scored result.** Commit
`4524c8f776943a267e04e03e9a237ecaed14bc2c`, tree
`4d5650a999ac0be59e71d5bd664e885cad5192c7`, retains result bytes
`sha256:ae0c980a18633832a83b73e0c715ee11e702aeb56660c4e027d5ece03425f372`
and canonical result
`sha256:92eed5bcb9e6b647d52a53282563077d3829b28c426e0dd9898a073f2590b8a5`.
Independent review `e6d8348bea3a57e88c5f9426d44a480b7a026fbd` passed the fixed negative
result with `positive_gate=not_supported`.

# 7. Reproduction contract

The paper's current source tree provides one entry point:

```bash
./paper/flagship/reproduce.sh
```

The command must fail closed unless it can resolve every bound commit and
reproduce the listed roots. It runs current Protocol 1 conformance, the
portable-divergence test, inherited-correction verification and deterministic
serialization checks, held-out custody and result checks, audit-manifest
reconstruction, and retained Erdős 264 unit checks in disposable detached
worktrees. It opens no protected adjudication and invokes no provider or
authority action.

The final public bundle must include every tracked source member plus the exact
external Formal Conjectures and Erdős source members needed for the real
correction check. A bundle qualifies as paper-ready after an independent task
reconstructs it from the manuscript commit and reproduces the manifest. An
outside group running the same bundle after publication would add external
reproduction; the current claim set does not require or presume that event.

# 8. Limitations

The empirical corpus contains four synthetic correction families and one
bounded real mathematical correction. Both model studies used one model family
and first-party infrastructure. The 16-session trial had eight sessions per
condition. The held-out study had 12 per arm across three families. Both
failed their positive gates. The first audit found output-contract and
navigation effects within its fixture; it cannot separate every model, packet,
or scorer effect.

The three-arm result measures structured presentation and Vela-specific
governance only within its frozen families and authority regimes. Prospective
length matching cannot make distinct representations identical, and the
neutral wrapper cannot test Repository authorization or Standing replay. Both
controls achieved perfect exact counts while Vela recorded one authority
error and a longer restricted mean. That result rejects the registered lift
claims for this benchmark; it does not establish a general Vela disadvantage.

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
post-result audit as a rescore. It does not turn the held-out gate failures
into positive lift or extrapolate them beyond the fixed synthetic benchmark.
It assigns no Repository, Decision, Event, Standing, or authority semantics to
the neutral wrapper. It does not use StateMem's results as Vela evidence. It
does not claim external reproduction, adoption, or general productivity.
Vela's global registry and derived global Frontiers do not constitute a single
global truth ledger.

# 10. Current paper-ready gate

The internal paper-ready gate consists of:

1. exact Protocol 1 and portable-divergence evidence;
2. a bounded controller/substrate trace and real correction case;
3. the sealed negative 16-session result and reviewed miss audit;
4. the protected held-out registration, fixed-denominator execution, and
   independent exact-byte negative-result review; and
5. a public-ready source and evidence bundle that passes the one-command
   reproduction contract.

Items 1 through 4 exist. This manuscript update binds the held-out terminal
result to item 5's one-command check. The remaining internal gate is an
independent exact-byte review of the reconciled paper and reproduction bundle.
No further model study is required to report the two negative empirical
results. External execution after publication can test the package under a new
operator and institution; it is not a prerequisite for this paper.

## 10.1 Remaining closure

The empirical program is terminal. The paper branch still requires one
independent review that reproduces the bound roots, verifies the claim matrix,
checks the PDF, and confirms that the one-command package performs no provider
or authority action. Publication and outside reproduction remain separate
future decisions.
