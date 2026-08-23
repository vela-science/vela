# What Inherited Scientific State Is For

Two preregistered held-out studies of structured scientific state, the boundary
between them, and two scoring artifacts we caught in our own results.

**Status: draft. Not submitted. Numbers are frozen; framing is not.**

## Abstract

Infrastructure projects for machine-scale science assume that presenting prior
work as structured, authority-bearing state helps whoever inherits it. We
tested that assumption twice, under preregistration and held-out adjudication,
and got opposite answers.

When a cold successor had to answer held-out questions about inherited work,
the structured presentation recovered 86.9% of supplied information against
77.4% for the same information as raw Git and source files, using 2.5 median
tool calls against 11.0. The raw-source arm produced two authority confusions
and two missed trust or licence dependencies; the structured arm produced
none.

When agents instead had to perform a bounded correction, structure bought
nothing. Git and documents were exact in 12 of 12 cells, a neutral structured
wrapper in 12 of 12, and the protocol presentation in 11 of 12. Every
preregistered gate was false, in every family.

The difference between the two is the contribution. Structured state does not
make agents better at tasks they can already do. It prevents a specific class
of error, about who accepted what and what superseded what, that raw sources
invite and that a task-success metric does not detect.

We also report two scoring artifacts our own instrumentation caught, both
arising from the same metric. A restricted-mean time metric substitutes a
600-second penalty for any failed cell. In a 16-cell precursor this produced an
apparent 5-of-8 against 0-of-8 win with a 2.6x speed advantage; in the 36-cell
study it produced an apparent 4.5x slowdown for the protocol arm. Neither was a
time measurement. Every run in both studies completed in between 10.95 and
20.94 seconds. We describe the failure mode because it is easy to make, and
because it is equally capable of manufacturing a win and manufacturing a loss.

## 1. The problem

Science collapses evidence into standing. A build passes, a check goes green,
a pull request merges, a paper is accepted, and each of those is read
downstream as licence to build on the result. When one of them turns out to be
wrong, nothing knows what to retract.

The information a successor needs is not preserved by any of the systems that
produce the work:

- the exact claim, at which source revision, with which limitations;
- what a check actually tested, and what it explicitly did not establish;
- who accepted or rejected the result, under whose authority;
- what has since been corrected, what it superseded, and what history remains.

That information lives in people's heads, in review threads, and in
convention. It does not survive a handoff between people, agents,
repositories, or time.

Vela is a Git-native protocol that records those four things and replays them
offline. Its one structural rule is that only an attributed Decision changes
accepted state: checks, merges, and signatures each keep their narrower
meaning. This paper does not argue for the design. It reports what happened
when we measured whether the recorded state helps anyone.

## 2. Two studies, one difference

Both studies were preregistered, used held-out adjudication frozen before any
session, and held supplied information constant across arms. Both are
internal. Neither has been externally replicated.

They differ in what the participant had to do.

| | Study A | Study B |
| --- | --- | --- |
| Task | answer held-out questions about inherited work | perform a bounded correction |
| Participant | cold successor, no prior context | agent with tools |
| Arms | Vela package, raw Git/source, native ecosystem | Vela, neutral state wrapper, Git/documents |
| Scale | 3 cases x 7 questions x 2 models | 3 families x 3 arms x 4 sessions |
| Outcome | information recovered | exact correction, impact completeness, authority safety, time |

Study B added a neutral structured wrapper as a third arm. The wrapper exposes
predecessor and successor links, dependencies, and active, superseded,
needs-recheck and current views, with no Repository, Decision, Event,
Standing, authority-scoped replay, or protocol vocabulary. It exists to
separate structure lift from governance lift. Without it, a positive result
cannot distinguish a protocol from a table of contents.

## 3. Study A: successor comprehension

Three correction-and-authority-heavy cases. Two models. Seven held-out
questions per case. The same supplied information in every arm, proved by an
input-equivalence receipt frozen before the runs.

| View | Points | Accuracy | Valid runs | Median time | Median tool calls |
| --- | ---: | ---: | ---: | ---: | ---: |
| Vela | 73 / 84 | 86.9% | 6 / 6 | 31.0 s | 2.5 |
| Raw Git / source | 65 / 84 | 77.4% | 6 / 6 | 44.2 s | 11.0 |
| Native ecosystem | 58 / 84 | 69.0% | 5 / 6 | 40.4 s | 4.0 |

One native session hung and was terminated at 1,371.618 seconds. It was not
retried and its seven answers stayed at zero. Excluding that entire paired
block leaves Vela at 88.6%, native at 82.9%, and raw Git at 77.1%. We report
both, because the exclusion changes the ordering of the two comparison arms
and not the position of the structured arm.

The error classes matter more than the totals. Within scored answers:

| Error class | Vela | Raw Git | Native |
| --- | ---: | ---: | ---: |
| Authority confusion | 0 | 2 | 0 |
| Correction-predecessor error | 0 | 0 | 1 |
| Missed trust or licence dependency | 0 | 2 | 0 |

Raw sources failed on exactly the things the protocol records. The navigation
cost points the same way: 2.5 median tool calls against 11.0 means the
structured arm was not searching for state the other arm had to reconstruct.

### Frozen roots

- Preregistration: `sha256:025fb027f948323afda33a846eb9f07dcf78a56f980b6e54fcfbc6728180bbc8`
- Math input commit `4624ea801c43b773b5d4a8b795c91e1882d8c02b`, tree `406a53dffcbb27fbb504987fb2dd8d565026abff`
- Candidate outputs frozen before held-out-key access: `sha256:563e5c5d0d785868de169ffea348dbb50d257936075c6bae99c6fe72f2caa2a4`
- Input-equivalence proof: `sha256:9601e687711e6fdd49515436327f88af0bd275aaf0083a73c27c7f332c9be99d`
- Pre-key leakage audit: `sha256:739bcfeb3ea22522d27253e72ae0c236d6154a7ff4b8cad7bcf7195f2d9b545f`
- Tool-trace audit: `sha256:1d5456baf24da1d8ecece9011e793662a5e52175b9e708b8bc03b7be1fb77dd4`
- Report: `sha256:0c07902449e9471b7ffaa3369d0c457d5cf0fa282263c4b9ad57550531ff235f`

## 4. Study B: correction inheritance

Three synthetic families: provenance revocation, taxonomy remap, method-version
correction. Twelve sessions per arm. Single-use permits, one attempt, zero
retries, zero substitutions. The adjudication key was held by an independent
evaluator and disclosed only as a canonical root until every cell was ingested.

| Arm | Exact | Impact-complete | Authority errors | Mean runtime | Restricted mean |
| --- | ---: | ---: | ---: | ---: | ---: |
| Git / documents | 12 / 12 | 12 / 12 | 0 | 12.80 s | 12.80 s |
| Neutral state wrapper | 12 / 12 | 12 / 12 | 0 | 13.98 s | 13.98 s |
| Vela | 11 / 12 | 12 / 12 | 1 | 14.63 s | 63.25 s |

The last two columns differ for one arm and the reason is important. The
preregistered restricted-mean metric substitutes a 600-second penalty for a
failed cell. Vela's single failure, `orderfix-run-25` in the taxonomy-remap
family, actually completed in 16.512 seconds and was scored at 600. The
arithmetic closes exactly: (175.538 - 16.512 + 600) / 12 = 63.252.

Reported as time, "4.5x slower" is false. Every one of the 36 runs completed
between 11.185 and 20.943 seconds. The honest statement is that the protocol
arm carried about 14% runtime overhead against raw Git and about 5% against
the neutral wrapper, and that it failed one cell.

Preregistered gates, aggregate and per family:

| Gate | Aggregate | provenance-revocation | taxonomy-remap | method-version-correction |
| --- | --- | --- | --- | --- |
| structure | false | false | false | false |
| governance_inheritance | false | false | false | false |
| total | false | false | false | false |

Estimands against their preregistered controls:

- structure (wrapper against Git/documents): exact success lift 0, authority error reduction 0.
- governance (Vela against wrapper): exact success rate lift -0.083, authority error rate reduction -0.083.

The task was nondiscriminative. Both comparison arms were exact in every cell,
so no arm could demonstrate an advantage, and the protocol arm paid overhead
for nothing. This is a property of the task, not a refutation of the mechanism.
Saying so is only legitimate because the gates were fixed before adjudication.
The forensic audit of run-25 found the response contract contains an
effect/action temporal ambiguity and that the mismatched response is
consistent with a current-safe-action reading; its cause remains unestablished,
and we do not claim it as a scorer defect, a capability failure, or grounds
for a rescore.

### Frozen roots

- Registration: `sha256:60acdfa31d25f9df5f342b75caf8e65426c5b71fa320c36fe5568de9fbf13b10`
- Adjudication: `sha256:26f5a7fb4ae0afcd4f0143e7efb9087b9dd05ff264590450d4361473deb2c39d`
- Capture: `sha256:f74229b3346cf56e2128d78b366f5fb99380872c27285d196c13862738bc8e98`

## 5. The boundary

Read together, the two studies give a scoped claim:

> Structured scientific state does not improve task success when the task is
> within the participant's reach. It removes authority and provenance errors,
> and cuts navigation cost substantially, when the successor has to reason
> about who accepted what, what superseded what, and what a check did not
> establish.

The practical reading for anyone building this kind of infrastructure: a
success-rate metric will not detect what the layer is for. Study B's arms were
indistinguishable on exactness, while Study A's arms differed by a factor of
four in navigation and produced four authority errors in one arm and none in
the other. Running only Study B, we would have concluded the layer was
useless. Running only Study A, we would have overclaimed.

## 6. Two artifacts from one metric

A 16-cell precursor to Study B reported:

| Arm | Exact | Authority errors | Restricted mean |
| --- | ---: | ---: | ---: |
| Vela | 5 / 8 | 3 | 233.08 s |
| Git / documents | 0 / 8 | 8 | 600.00 s |

Restricted-mean ratio 0.388. Read naively, that is a large win: five successes
against none, at 2.6 times the speed.

Neither half survives inspection.

**The time column is not time.** Git's 600.00 is exactly eight 600-second
penalties averaged. The eight Git cells actually completed in 11.45, 11.48,
11.57, 12.20, 12.31, 12.79, 13.94 and 17.29 seconds. The eight Vela cells
completed in 10.95 to 15.59 seconds. The arms were within about half a second
of each other in real time. Nothing timed out.

**The success column is not participant behaviour.** The project's own
post-result audit attributes every failed cell in both arms to a fixture or
scorer limitation, records zero model-capability failures on the substantive
task, and counts 8 source-binding digest misses and 11 verbose semantic-none
misses. The Git arm's failure cause is recorded as
`fixture_scorer_limitation_and_representation_navigation` in all eight cells.

The preregistered gate returned `not_supported` even on these numbers, and the
recalibrated 36-cell replacement in Section 4 shows the effect was not there.

The same metric then produced the opposite artifact in Study B, where a single
16.5-second failed cell became an apparent 4.5x slowdown. A penalty-encoding
composite metric is legitimate and we keep it, because a failure should cost
something. But it must never be read or reported as a duration, and a
difference in it must be decomposed before it is interpreted. Both of our
apparent effects, one favourable and one unfavourable, were this.

## 7. Limitations

- Both studies are internal. Neither has been externally replicated.
- Study A covers three cases and two models. Study B covers three synthetic
  families constructed for the purpose.
- Neither study establishes mathematical truth, source fidelity, maintainer
  acceptance, cross-domain performance, model-independence, general
  productivity, adoption, or any authority or Standing effect.
- Study B's families are synthetic. Real corrections may distribute
  differently.
- Study A's native arm lost a session to a hang; we report with and without
  that paired block.
- Study A and Study B differ in task, participant, arms and scoring. The
  boundary in Section 5 is an interpretation of two studies, not a
  manipulation of one variable within a single design.

## 8. What would change our minds

The open gates are outside the implementation and none of them have passed:

1. an outside producer completes a real result through the loop;
2. an outside scoped checker returns for a second result;
3. a separately operated authority makes its own decision;
4. one real correction is discovered and inherited by later work;
5. a broader preregistered evaluation reproduces or overturns Study A.

Until then the honest description is a working, internally qualified
inheritance mechanism with one positive and one negative preregistered result,
and a boundary between them that we did not predict in advance.
