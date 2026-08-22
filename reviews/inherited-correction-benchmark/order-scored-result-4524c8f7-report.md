# Independent final review of the order-contract scored result

## Verdict

**PASS** for producer commit
`4524c8f776943a267e04e03e9a237ecaed14bc2c`, tree
`4d5650a999ac0be59e71d5bd664e885cad5192c7`, with sealed-capture parent
`5694bebac03b062d6acdce5a2a900551850e6a1c`.

The imported result and its non-plaintext custody evidence reproduce exactly.
The result is `positive_gate=not_supported`; structure,
governance/inheritance, and total gates are all false. This is a result review,
not a Vela Decision or scientific acceptance action.

## Immutable subject and scope

- Live `origin/codex/inherited-correction-study` resolved exactly to the
  reviewed commit and tree; its immediate parent is the independently audited
  sealed capture.
- The producer delta adds exactly four files, all under
  `paper/artifacts/inherited-correction-held-out-order-replacement-result/`:
  the scored result, non-plaintext scoring access receipt, validation receipt,
  and result manifest.
- The registered benchmark artifact and complete execution/capture directory
  retain exactly the same Git tree objects as the parent.
- No provider events, participant responses, permits, capture files, gates,
  Core, Protocol, Repository authority, Standing, or Decision bytes changed.
- None of the four added files has the protected object's 5,883-byte length or
  protected SHA-256. The protected adjudication plaintext was not imported or
  committed.

## Roots and custody

Independent byte and canonical-root checks reproduced:

- result evidence root:
  `sha256:d9f017734d1c58ca9ecaba2621a7ddec12e178a78bf6b2d228dc2542aa71a104`;
- scored-result bytes:
  `sha256:ae0c980a18633832a83b73e0c715ee11e702aeb56660c4e027d5ece03425f372`;
- scored-result canonical root:
  `sha256:92eed5bcb9e6b647d52a53282563077d3829b28c426e0dd9898a073f2590b8a5`;
- scoring access receipt root:
  `sha256:b63efee54976a9d6da866a49f12c956621e4da4540a04306d3d0f81ca2d7b3b3`;
- immutable score snapshot root:
  `sha256:f74229b3346cf56e2128d78b366f5fb99380872c27285d196c13862738bc8e98`;
- sealed capture root:
  `sha256:4a592d88b43dc02d5495d7679834535d6fa97f20759600400253677a946f87fd`;
- complete custody root:
  `sha256:ccf69e70a3887c8a9f9ddffa2d62051e114a8974b2d2ae83c72366a1eb98dcef`.

The access and validation receipts consistently record one authorized scoring
process and one scoring attempt, zero provider calls during scoring, zero
capture/participant/gate mutations, no retained temporary plaintext copy, and
`authority_effect=none`. The review did not invoke `benchmark.py score` or
`score_runs`; the registered scorer therefore remains at exactly one use.

## Independent recomputation

A standalone verifier implemented the registered closed response comparison,
family/condition grouping, restricted-time rule, estimands, half-even decimal
quantization, family gates, aggregate gates, and deterministic JSON
serialization without importing or calling the committed scorer.

It read the evaluator object in place, checked its committed digest and
canonical root, retained no copy, and emitted no protected content. It then:

1. reconstructed the scoring snapshot from the 36 sealed custody entries;
2. verified every run and response byte digest and every unique run and
   participant identity;
3. validated every closed response field, consequence ID, authority code, and
   packet evidence binding;
4. recomputed all 36 per-run exact-success, correction-impact-completeness,
   pair, and authority comparisons;
5. recomputed every family/arm and aggregate metric, restricted time,
   estimand, family gate, aggregate gate, and positive gate;
6. serialized the complete result independently.

The independently generated result object equals the imported result at every
field and regenerates its 11,145 bytes byte-for-byte. This also confirms all
per-family estimands and every decimal/rounding boundary, not only the
headline metrics.

The requested high-risk values reproduce exactly:

- taxonomy-remap Vela: 3/4 exact, one authority error, all 4/4 correction
  impacts complete, restricted mean `163.93218100625` seconds;
- aggregate exact successes: Git/documents 12/12, state wrapper 12/12, Vela
  11/12;
- aggregate correction-impact complete: 12/12 in all three arms;
- aggregate restricted means: Git/documents `12.800895867`, state wrapper
  `13.98268798558333`, Vela `63.252235329` seconds;
- structure gate false, governance/inheritance gate false, total gate false,
  governance strict increment false, and `positive_gate=not_supported`.

## Focused checks

- Exact producer commit/tree/parent/live-remote equality: PASS.
- Four-file result-only delta and unchanged artifact/capture trees: PASS.
- Registered benchmark deterministic verification without scoring: PASS.
- Independent 36-run scoring and arithmetic recomputation: PASS.
- Independent complete-result byte regeneration: PASS.
- Result, receipt, manifest, snapshot, capture, and custody roots: PASS.
- One-scoring-process/one-attempt and zero provider/mutation assertions: PASS.
- Protected-plaintext absence check: PASS.
- `git diff --check`: PASS.

## Claim ceiling

The evidence is one fixed, synthetic, internally run 36-cell held-out
benchmark. Every preregistered positive gate was not supported. It establishes
no scientific acceptance, external replication, broad productivity claim,
Protocol or Core change, Repository authority, Standing, or Decision effect.

This PASS confirms only the immutable scored result and its custody evidence.
It does not authorize a provider call, participant rerun, evidence or gate
mutation, merge, scientific acceptance, or authority action.
