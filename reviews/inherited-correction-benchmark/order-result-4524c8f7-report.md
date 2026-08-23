# Independent final orderfix held-out result review

## Verdict

**PASS**, bound to result commit
`4524c8f776943a267e04e03e9a237ecaed14bc2c`, tree
`4d5650a999ac0be59e71d5bd664e885cad5192c7`, whose sole parent is the
sealed-capture commit `5694bebac03b062d6acdce5a2a900551850e6a1c`, tree
`feec0ff21b9b13be8cbb97083f441ef66bdd48f2`.

This PASS validates the exact retained 36-cell result, its sealed participant
custody, fixed denominator, result serialization, arithmetic, registered gate
evaluation, and bounded claim. The primary outcome is
`positive_gate=not_supported`; this review does not support a Vela lift claim.
It does not authorize a rerun, rescore, provider call, merge, scientific
acceptance, Protocol or Core change, Repository authority, Standing, or a
Decision effect.

## Immutable subject and score boundary

The producer ref was re-fetched and remained remote-equal at the exact result
commit and tree. The result commit adds only four files under
`paper/artifacts/inherited-correction-held-out-order-replacement-result/`:

- `scored-result.json`;
- `scoring-access-receipt.json`;
- `validation-receipt.json`; and
- `manifest.json`.

No participant run, response, event, permit, receipt, capture, registration,
runtime, scorer, packet, prompt, schema, gate, Core, Protocol, Standing,
authority, or Decision byte changed after the sealed parent. The result commit
has one parent and is not an ancestor of live `origin/main`, which was
`cc3b88d8bfcfd7b4f720a023f049d5c365be9423` at final refresh.

The cited independent pre-score PASS is reachable exactly at
`b634523ea1c85dce697404968cf7492f09a6412f` on
`origin/codex/review-order-prescore-5694beba`. It binds the same sealed parent,
complete capture, and complete custody roots.

This reviewer did not invoke `benchmark.py score`, import protected
adjudication plaintext, or inspect an answer map. Aggregate validation below
uses only the committed scored family summaries and sealed public run/custody
bytes.

## Sealed execution and custody

Independent reconstruction from all run directories reproduced the committed
complete-custody object byte-for-byte and the following roots exactly:

- complete capture:
  `sha256:4a592d88b43dc02d5495d7679834535d6fa97f20759600400253677a946f87fd`;
- complete custody:
  `sha256:ccf69e70a3887c8a9f9ddffa2d62051e114a8974b2d2ae83c72366a1eb98dcef`;
- score capture:
  `sha256:f74229b3346cf56e2128d78b366f5fb99380872c27285d196c13862738bc8e98`;
- score-capture manifest bytes:
  `sha256:e96cd9f29b8c58434480080e3f52d6e10eb2dea8147d83beede131fdf5a56a85`;
- registration:
  `sha256:60acdfa31d25f9df5f342b75caf8e65426c5b71fa320c36fe5568de9fbf13b10`;
  and
- assignment:
  `sha256:64a356db4800b6fb04090ae81a6c2d33bf37ad8b71e92e01567edc5fa6362e72`.

The outer capture manifest has 362 unique entries and exactly covers every
sealed execution file except itself. Every byte length and SHA-256 digest
matches. All 36 expected run directories pass the registered custody verifier.

The execution independently recomputes as:

- 36 terminal `completed` runs and 36 distinct participant identities;
- 12 Git/documents, 12 state-wrapper, and 12 Vela sessions;
- 12 sessions per family and four per family/arm cell;
- 36 distinct authorized consumed permits, each attempt one and exactly bound
  to its frozen run, participant, assignment, configuration, packet, prompt,
  image, trust, and runtime identities;
- permit consumption before provider start for every run;
- strictly sequential timestamp intervals from run 01 through run 36;
- one response and one turn per run, zero tool calls and compactions;
- zero timeouts, validation errors, retained credentials, retries, or
  substitutions; and
- byte-equality between each retained raw runtime response and its ingested
  `response.json`.

Git history contains exactly 36 consecutive one-run retention commits followed
by the complete-capture freeze. Each run was therefore retained before its
successor.

The two stopped predecessor registrations remain immutable one-run
non-results, with 35 unissued cells, zero retry/substitution/scoring, false
replacement credit, and continuation forbidden. The fresh neutral order
calibration remains a distinct attempt-one calibration with
`calibration_denominator_credit=false`. Neither stopped run nor any calibration
appears among the 36 orderfix capture entries or receives score credit.

## Result bytes and access receipt

Independent hash and canonical-JSON reconstruction produced:

- result bytes:
  `sha256:ae0c980a18633832a83b73e0c715ee11e702aeb56660c4e027d5ece03425f372`;
- result canonical root:
  `sha256:92eed5bcb9e6b647d52a53282563077d3829b28c426e0dd9898a073f2590b8a5`;
- scoring-access receipt bytes:
  `sha256:a93ade57ea0c53dddd49c8a28ab9b10de0004760069075ab72c558572481eb6c`;
- scoring-access receipt root:
  `sha256:b63efee54976a9d6da866a49f12c956621e4da4540a04306d3d0f81ca2d7b3b3`;
- validation-receipt bytes:
  `sha256:ab4bf91b02502849840bb1c470796c611db5c410603ef86874cee9eb64f6677e`;
  and
- result-evidence root:
  `sha256:d9f017734d1c58ca9ecaba2621a7ddec12e178a78bf6b2d228dc2542aa71a104`.

The access and validation receipts consistently bind one scoring process, one
scoring attempt, the exact pre-score PASS, the three capture/custody roots,
zero scoring-time provider calls, zero participant/capture/gate mutation, no
retained temporary plaintext, and no committed protected object. No second
result, access receipt, or result process is represented in the immutable
artifact. This establishes the one-opening claim to the extent recorded by the
evaluator-custody receipt; the repository itself contains only the commitment,
not plaintext independently available for a second scoring audit.

## Independent aggregate and gate arithmetic

Using Decimal `ROUND_HALF_EVEN` with the registered mean and ratio quanta, the
three committed family blocks independently roll up to:

| Arm | Sessions | Exact | Impact-complete | Authority errors | Restricted mean seconds |
| --- | ---: | ---: | ---: | ---: | ---: |
| Git/documents | 12 | 12 | 12 | 0 | 12.800895867 |
| State-wrapper | 12 | 12 | 12 | 0 | 13.98268798558333 |
| Vela | 12 | 11 | 12 | 1 | 63.252235329 |

All reported rates, aggregate estimands, family gates, and aggregate gate
arithmetic are internally consistent. The single Vela authority error leaves
correction-impact completeness at 12/12 but makes exact success 11/12 and
assigns the registered 600-second restricted outcome to that miss.

The registered gates independently recompute as:

- structure: `false`;
- governance/inheritance: `false`;
- total: `false`;
- governance strict increment: `false`; and
- positive gate: `not_supported`.

The result's `authority_effect` is exactly `none`.

## Focused checks

- exact remote commit/tree/parent and pre-score review identity: PASS;
- four-file additive-only result scope and no capture mutation: PASS;
- all 36 terminal custody and permit records: PASS;
- complete custody byte equality and three capture/custody roots: PASS;
- 362-entry outer capture manifest coverage and hashes: PASS;
- strict run chronology, fixed 12/12/12 balance, and zero retry/substitution:
  PASS;
- result, access receipt, validation receipt, and result manifest hashes/roots:
  PASS;
- independent Decimal aggregate and gate arithmetic: PASS;
- registered benchmark verification and prelaunch-template verification: PASS;
- 24 benchmark tests and 9 provider-runtime tests, without protected key
  access: PASS;
- Ruff 0.12.11 lint: PASS; and
- result diff whitespace check: PASS.

Ruff's formatting-only check would reformat four unchanged inherited Python
files. Those files predate the sealed capture and result commit, the lint and
deterministic suites pass, and the result diff does not touch them; this is not
a result-integrity or methodology blocker.

## Claim ceiling

This is one fixed, synthetic, internally run 36-cell benchmark. All
preregistered positive gates are unsupported. The exact result does not show a
Vela advantage over either comparison arm and does not establish scientific
truth, external replication, general productivity, adoption, acceptance,
Protocol or Core validity, Repository authority, Standing, or a Decision
effect.
