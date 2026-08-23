# Current evidence and open validation gates

This page separates demonstrated software behavior, internal evaluation, and
external validation. It is a status record, not a scientific Claim.

## Demonstrated product behavior

The signed `v0.977.4` CLI can create a native Repository, authenticate a
Submission, retain a scoped Verification Record, record an attributed accept
or reject Decision, recover an interrupted transaction, replay exact state,
and explain current Standing.

The public [Vela Math Repository](https://github.com/vela-science/math) at
commit `5de716c896065c03c0a470d015ba2a328a527f73` replays strictly to Repository
root
`sha256:a956b84c437202e5a02cc9e036a621bd14a302b34a75758115730bdbb77c52a4`.
It contains 3 current accepted bounded Claims, 6 Submissions, 6 Verification
Records, and no pending review.

These facts demonstrate the mechanism. They do not demonstrate external
adoption, plural authority, or scientific truth.

## Phase 0 inheritance evaluation

On 2026-08-20, the project completed a preregistered, same-information,
three-case cold-successor comparison. Two models answered seven held-out
questions for each case under three presentations: a Vela package, raw
Git/source files, and a native-ecosystem view.

| View | Points | Accuracy | Valid runs | Median time | Median tool calls |
| --- | ---: | ---: | ---: | ---: | ---: |
| Vela | 73 / 84 | 86.9% | 6 / 6 | 31.0 s | 2.5 |
| Raw Git/source | 65 / 84 | 77.4% | 6 / 6 | 44.2 s | 11.0 |
| Native ecosystem | 58 / 84 | 69.0% | 5 / 6 | 40.4 s | 4.0 |

One native Erdős 321 session hung and was terminated after 1,371.618 seconds.
It was not retried and its seven answers remained zero. Excluding that entire
paired block leaves Vela at 88.6%, native at 82.9%, and raw Git at 77.1%.

Within the scored answers, Vela had no observed authority confusion,
correction-predecessor error, or missed trust/licence dependency. Raw Git had
two authority confusions and two missed trust/licence dependencies. Native had
one correction-predecessor error plus missing answers from the failed session.

### Frozen evidence roots

- Preregistration:
  `sha256:025fb027f948323afda33a846eb9f07dcf78a56f980b6e54fcfbc6728180bbc8`
- Math input commit:
  `4624ea801c43b773b5d4a8b795c91e1882d8c02b`
- Math input tree:
  `406a53dffcbb27fbb504987fb2dd8d565026abff`
- Candidate outputs frozen before held-out-key access:
  `sha256:563e5c5d0d785868de169ffea348dbb50d257936075c6bae99c6fe72f2caa2a4`
- Input-equivalence proof:
  `sha256:9601e687711e6fdd49515436327f88af0bd275aaf0083a73c27c7f332c9be99d`
- Pre-key leakage audit:
  `sha256:739bcfeb3ea22522d27253e72ae0c236d6154a7ff4b8cad7bcf7195f2d9b545f`
- Tool-trace audit:
  `sha256:1d5456baf24da1d8ecece9011e793662a5e52175b9e708b8bc03b7be1fb77dd4`
- Final internal review:
  `sha256:3c6e63712238fab5c078fd04777557d429f66279b2c5d85e68b8cc9e3d0293e3`
- Phase 0 report:
  `sha256:0c07902449e9471b7ffaa3369d0c457d5cf0fa282263c4b9ad57550531ff235f`

The independent review rated the corrected bundle ready for internal sharing.
It was an internal review, not an external replication.

## What Phase 0 supports

The result supports one bounded statement: for these three
correction-and-authority-heavy packets, the Vela presentation helped cold
successors recover more supplied information with less navigation.

It does not establish:

- a general causal productivity improvement;
- mathematical truth or source fidelity;
- maintainer acceptance;
- performance across scientific domains;
- model-independent effects;
- external validation; or
- a Vela authority role.

## Held-out correction study

On 2026-08-21 the project completed a second preregistered, held-out study on
a different task. Where Phase 0 asked a cold successor to answer questions
about inherited work, this study asked an agent to perform a bounded
correction. Three synthetic families, three arms, four sessions each. The
adjudication key was held by an independent evaluator and disclosed only as a
canonical root until every cell was ingested. Single-use permits, one attempt,
zero retries, zero substitutions.

| Arm | Exact | Impact-complete | Authority errors | Mean runtime |
| --- | ---: | ---: | ---: | ---: |
| Git / documents | 12 / 12 | 12 / 12 | 0 | 12.80 s |
| Neutral state wrapper | 12 / 12 | 12 / 12 | 0 | 13.98 s |
| Vela | 11 / 12 | 12 / 12 | 1 | 14.63 s |

Every preregistered gate was false, aggregate and per family: `structure`,
`governance_inheritance`, and `total`. The task was nondiscriminative — both
comparison arms were exact in every cell, so no arm could demonstrate an
advantage.

- Registration root: `sha256:60acdfa31d25f9df5f342b75caf8e65426c5b71fa320c36fe5568de9fbf13b10`
- Adjudication root: `sha256:26f5a7fb4ae0afcd4f0143e7efb9087b9dd05ff264590450d4361473deb2c39d`
- Capture root: `sha256:f74229b3346cf56e2128d78b366f5fb99380872c27285d196c13862738bc8e98`

## Reading the two studies together

They do not conflict; they differ in task. Structured state did not improve
task success when the task was within the participant's reach. It removed
authority and provenance errors, and reduced navigation cost, when the
successor had to reason about who accepted what and what superseded what. In
Phase 0 the raw-source arm produced two authority confusions and two missed
trust or licence dependencies and the Vela arm produced none, at 2.5 median
tool calls against 11.0.

A success-rate metric does not detect what the layer is for. Reporting only
the held-out correction study would understate it; reporting only Phase 0
would overstate it.

## A metric caution

Both studies score a failed cell by substituting a 600-second penalty into a
restricted-mean time statistic. That composite is deliberate, and a failure
should cost something, but it must never be read or reported as a duration.

It has produced a misleading number in both directions here. In the 16-cell
precursor to the correction study it produced an apparent 2.6x speed
advantage for Vela; the comparison arm's cells actually completed in 11.45 to
17.29 seconds and its 600.00 figure was eight penalties averaged. In the
36-cell study it produced an apparent 4.5x slowdown for Vela; the single
failed cell, `orderfix-run-25`, actually completed in 16.512 seconds and was
scored at 600, and the arithmetic closes as
(175.538 - 16.512 + 600) / 12 = 63.252. No run in either study exceeded 21
seconds.

Decompose any difference in this statistic before interpreting it.

## Open validation gates

The most important missing evidence is outside the Core implementation:

1. an outside producer completes a real Result through the loop;
2. an outside scoped checker returns for a second Result;
3. a separately operated Repository authority makes its own Decision;
4. one real correction is discovered and inherited by later work; and
5. a broader preregistered evaluation reproduces or overturns Phase 0.

Until those gates pass, describe Vela as a working and internally qualified
scientific-inheritance mechanism with one positive and one negative
preregistered internal result, not a mature or externally validated scientific
platform.
