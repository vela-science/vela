# Independent inherited-correction post-result miss-audit review

## Verdict

**PASS**, bound to producer commit
`de13073ff8f3a9f2958f8c93c848205c533ddb1e`, tree
`0368fac90944a23cbd71e84589f4d84d4aba678e`, whose sole parent is the
independently passed canonical-result commit
`7641d775911f6026a9c36649d6cf1354dd1f70c0`.

This PASS qualifies only the exact bytes, per-cell reconstruction,
fixture-bounded causal classification, repository boundary, and prospective
claim ceiling of this non-authoritative audit. The preregistered primary result
remains `positive_gate=not_supported`; the audit does not rescore or reinterpret
that result. It authorizes no provider/model call, participant rerun, retry,
substitution, implementation, merge, scientific claim, Protocol/Core change,
Decision, authority action, or Standing effect.

## Exact scope and bindings

The remote producer ref resolved exactly to the handed-off commit, tree, and
parent. The commit adds only:

- `audit.json`, bytes
  `sha256:2bbff6b3d96cec8b17e84057c3ed50b37fc5504ee54faef29213f1b4cf52c1d6`;
- `audit.md`, bytes
  `sha256:29af4fb78e08f83430d13004dbbba10010d69a611b4fc55a8be43e773ea14cc4`;
  and
- `manifest.json`, bytes
  `sha256:c64a4e5da2e3c9d1ea641b78ce63ba1479a480ca8a8100e925ba0a201f3e2d30`.

The two manifest file hashes independently match. Canonical compact/sorted JSON
of the files-only object, `{"files": [...]}`, reproduces artifact root
`sha256:8463024ee31116c33cee9e43262286bb78855654ecc974e77818bf4dfac581af`.
This is the committed manifest's “files field only” scope; timestamps, schema,
root description, and the root itself are excluded.

The audit's external bindings independently match:

- canonical result bytes
  `sha256:48c3ab674e1ef707a207c2a5cf8addab16d7209e8229def76f0f1568a466f83f`;
- capture root
  `sha256:0e5f60fa1dc78e531d44cb8fff626e73c6b2c0017bbcec52e41220cbfac686fd`;
- complete runtime custody root
  `sha256:619512f17009dd92c651a687cbc17dd5899c0b908619d82de465b9747a7aa3f5`;
- protected adjudication root
  `sha256:6b2e94c7bfce7c41353eb48cd4962243e3f177fdaccb8c7da48567d99dfca557`;
  and
- independent F06 PASS review
  `1f7ebabee72058619e8081d71c3fc4325b81f64b`.

No parent result, run, response, event, permit, packet, scorer, key, capture, or
Core/Protocol byte changed.

## Per-cell reconstruction

The protected key and all 16 sealed responses were independently read after
the result was already sealed. Every audit row reproduces the registered
scorer exactly:

- all 16 predecessor/successor pairs are exact;
- all 64 consequence classifications are exact;
- all 64 required action codes are exact;
- the five Vela exact successes are runs 01, 03, 06, 09, and 14;
- Vela totals are 130 points, five exact successes, and three authority errors;
- Git/documents totals are 112 points, zero exact successes, and eight
  authority errors; and
- every exact success retains actual duration while every miss retains the
  registered 600-second restricted outcome.

The categorical result, means, fixed 8+8 denominator, `not_supported` gate,
and `authority_effect=none` match the canonical result.

## Miss classification

The eleven standing-field misses all explicitly state the semantic conclusion
that there is no Decision, acceptance, validation, authority, or Standing
effect. Five other Vela runs express the same conclusion as literal `none`.
The response schema allowed any nonempty string and the task asked participants
to “state” the effect, while the protected scorer awarded authority correctness
only when `standing_effect.casefold() == "none"`. Classifying the eleven misses
as a fixture/scorer contract limitation, rather than evidenced authority
misunderstanding, is therefore supported by the sealed text.

All eight Git/documents binding fields cite existing, relevant source/evidence
paths and accurately describe the cited contents, but include no SHA-256
digest. The scorer required a known digest substring even though the schema
again required only a nonempty string and the task did not state that hidden
literal requirement. The Git packet contains the same digests in
`PACKET-MANIFEST.json`; the Vela packet colocates them in per-Claim binding
objects, and every Vela response includes an accepted digest. The systematic
8/8 versus 0/8 split supports the audit's representation/navigation diagnosis,
with output-contract ambiguity as a second cause.

These causal labels are proportional only within this sealed synthetic fixture.
They do not establish a general model-capability result, a general Vela
advantage, or a counterfactual performance estimate. The audit preserves that
ceiling: it keeps the registered miss penalties and `not_supported` result
primary and describes no substantive correction/classification/action failure
in the observed 16 cells.

## Boundary and prospective design

The recommended contract repair belongs to a fresh benchmark, not Vela Core:
a closed generic standing-effect code, structured `{path, sha256}` binding
validated against the assigned packet, the same schema for both arms, and
separate semantic/time reporting. Nothing in the observed failure requires a
Protocol object, canonical encoding, replay, Repository authority, Decision,
Event, or Standing change.

The proposed follow-up remains only a design recommendation. It uses three new
topology/vocabulary families, two arms, and four fresh instances per cell for
24 fixed sessions; fresh seed, participants, assignments, and protected key;
one permit at a time; zero retries; evaluator custody; byte/root commitment
before execution; release after all captures seal; offline fail-closed
adversaries; and independent prelaunch review. It assumes no positive outcome
and does not claim statistical sufficiency or scientific acceptance. Any use
requires a separate prospective registration and authorization.

## Focused checks

The following passed from a fresh detached checkout:

- exact remote commit/tree/parent and three-path diff;
- all three file hashes and the files-only artifact root;
- canonical result, capture, custody, adjudication, and F06-review bindings;
- registered scoring recomputation for all 16 cells;
- exact pair, classification, action, standing, binding, points, success,
  authority-error, duration, and restricted-duration comparisons;
- source/evidence path existence for all eight path-only Git bindings;
- benchmark verification;
- all 16 benchmark tests; and
- `git diff --check`.

There are no blocking findings on this audit commit.
