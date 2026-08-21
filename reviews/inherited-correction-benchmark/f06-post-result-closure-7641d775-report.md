# Independent F06 post-result serialization re-review

## Verdict

**PASS**, bound to producer commit
`7641d775911f6026a9c36649d6cf1354dd1f70c0`, tree
`b1cd7fa343a7bc5fd3741cafecda2d1034dccc4c`, whose sole parent is the
original result commit `3207066f22f09b578f354b7028f55559e7b45926`.

This PASS closes only F06: deterministic serialization of the already sealed
post-result aggregates and the corresponding narrow claim ceiling. It
qualifies canonical result bytes
`sha256:48c3ab674e1ef707a207c2a5cf8addab16d7209e8229def76f0f1568a466f83f`
for the unchanged capture. It does not claim positive lift or scientific
acceptance and authorizes no participant rerun, retry, substitution, provider
call, merge, Protocol/Core change, Decision, authority action, or Standing
effect.

## Exact scope and amendment

The pushed producer ref resolved exactly to the handed-off commit, tree, and
parent. Its diff contains exactly five declared paths: the executable
canonicalizer, executable test, amendment, serialization fixture, and corrected
result. Their Git modes and SHA-256 hashes match the handoff.

The amendment transparently binds:

- original result commit `3207066f22f09b578f354b7028f55559e7b45926`,
  tree `451237b4b85df33da5b8d8442fe67bd60b8d3b08`, and result bytes
  `sha256:1f1d886c778e8fef0effce59692f761eb6d937afa9421880aed3340079004679`;
- independent BLOCKED review
  `53fda88b94bc4b2d025c0d42d5fd1ac26441e401` and the exact report/verdict
  byte hashes;
- the observed CPython 3.10–3.13 alternate result bytes;
- the sealed capture and complete-custody roots;
- zero participant calls after the original result, no evidence change, no
  retry/replacement, and no protected scientific-rule change; and
- the canonical decimal policy and corrected result bytes.

Independent reconstruction of the review commit reproduced the amendment's
prior report and verdict hashes exactly.

## Canonical arithmetic and scoring custody

The wrapper preserves the passed pre-key order. It first runs the registered
capture verification, then obtains the no-follow immutable run/response
snapshot and reconstructs its capture root. Only after those checks does it
open protected adjudication. Response scoring continues to use the unchanged
registered `score_response` implementation and unchanged protected key.

Run durations are parsed directly as `Decimal`, restricted seconds are summed
from `Decimal(0)`, and the canonical aggregates use `ROUND_HALF_EVEN` with:

- mean quantum `0.00000000000001`; and
- ratio quantum `0.000000000000001`.

Only those aggregate serializations differ from the original interpreter-
dependent boundary. The corrected result changes the ratio from
`0.38846373067458334` to `0.388463730674583`; the Vela mean remains
`233.07823840475`. The fixed denominator, condition counts, points, exact
successes, authority errors, median tools, capture/adjudication/registration
roots, limitations, positive gate, and authority effect are unchanged.

The positive gate remains `not_supported`; authority effect remains `none`.

## Cross-version reproduction

Independent runs under available CPython 3.10, 3.11, 3.12, 3.13, and 3.14
each reproduced:

- fixture metrics bytes
  `sha256:0edc8b9adea2302c60ac988c9a27c0b5e7c3148152ecbae4dcb41fb613159473`;
  and
- full canonical result bytes
  `sha256:48c3ab674e1ef707a207c2a5cf8addab16d7209e8229def76f0f1568a466f83f`.

Every temporary full output was byte-identical to the committed corrected
result. These review invocations read the protected key only after independently
verifying the sealed capture. They made no provider/model call and changed no
participant, permit, auth, capture, or authority state.

## Unchanged evidence and roots

No participant run, response, event, permit, raw capture, or capture-manifest
byte differs from the original result parent. The committed capture manifest
remains bytes
`sha256:f8347ca31241de45de7b8867619cca3c51bdf7334ba48904871522227a76155e`.
Independent capture verification, before adjudication, reproduced:

- capture root
  `sha256:0e5f60fa1dc78e531d44cb8fff626e73c6b2c0017bbcec52e41220cbfac686fd`;
- complete runtime custody root
  `sha256:619512f17009dd92c651a687cbc17dd5899c0b908619d82de465b9747a7aa3f5`;
  and
- `adjudication_accessed=false` in the capture manifest.

The original result and corrected result differ in exactly the single ratio
line described above. Registered benchmark implementation, tests, scoring key,
packets, prompts, schema, assignments, runtime, and evidence are unchanged.

## Focused checks

The following passed from a fresh detached checkout:

- exact remote commit/tree/parent, five-path diff, modes, and file hashes;
- prior review report/verdict binding reconstruction;
- unchanged capture and participant-evidence comparison;
- pre-key capture verification;
- fixture checks under CPython 3.10–3.14;
- serialization unit tests under CPython 3.10–3.14;
- full same-capture regeneration and byte comparison under CPython 3.10–3.14;
- Ruff 0.12.11 check and format check on both Python files;
- benchmark verification;
- all 16 benchmark tests; and
- `git diff --check`.

There are no remaining F06 findings on the reviewed commit.
