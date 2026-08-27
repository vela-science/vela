# Research-program loop dogfood

This is a product dogfood record, not a scientific causal trial. It evaluates
whether one read-only Vela command can recover the current Vela machine-science
program without reading the originating chats.

## Exact invocation

```bash
cargo run --locked -q -p vela-cli -- \
  integration program examples/research-program/2026-08-27-dogfood.json

cargo run --locked -q -p vela-cli -- \
  integration program examples/research-program/2026-08-27-dogfood.json --json
```

The dogfood input root is
`sha256:451790336f952476dcb6439d11d2a1ac9e5a9ae40c50697a1319784d1bfd9b0a`.
The view is bound to `2026-08-27T22:04:04Z`; later task state requires a fresh
input observation rather than reinterpretation of this snapshot.

## Source custody

All seven declared source files matched their expected SHA-256:

| Source | SHA-256 |
| --- | --- |
| Longitudinal synthesis | `d0e62bf27f2ccc0d02cbd7ff26cb057b87242dff42714cace8b2bde2b9501ae5` |
| Research episode index | `36a6c4ead91c3909cb8bb271376de5b15fae91c89a8d32a9afbe7317cddf55c8` |
| Experiment ledger | `62c1bc74884b50c63909133d8ababfbe1e7fba7b3cd70c4ae0061b55d8fca382` |
| Hypothesis register | `b15dfb23bc55c30a95a62d516e1f4ef72e1fb575a92191bfe0f7d716c47c036c` |
| H-112 denominator | `fe0af58b0e1702b7edca2d76dcffc9a16a9b818e02d5470c0b8f917e6a68661e` |
| H-112 analysis | `03058b2b27061e7664c7848eacf8039e4efcb0e95a1b4145d1bf5e8c555156a7` |
| H-111 retrospective | `731e926e9f207c430dc933775237a3ab44220e6b979b1af3db8709dcc03fdcb9` |

The input stores only metadata, source pointers, compact task observations,
and proposed changes. It does not retain chat transcripts, prompts, private raw
traces, credentials, or source artifact bytes.

## Cold-successor result

One human-mode command reports the thesis, full denominator counts, supervision
exceptions, proposed updates, next action, and authority ceiling. JSON exposes
the complete query paths:

| Cold-successor question | Output field |
| --- | --- |
| What was tried? | `denominator.episodes` (58 rows) and `denominator.material_attempts` (14 rows) |
| What succeeded, failed, was killed, stopped, invalid, null, or remains open? | `denominator.status_counts`, `denominator.category_counts`, and each episode's `result_status` |
| What exact evidence supports it? | each episode's `evidence_pointer` plus `evidence.checks` |
| Which tasks need a choice or callback? | `attention.tasks` |
| What should happen next and why? | `next_action` |

The denominator remained 58 unique campaigns and 14 material nested attempts:

| Disposition | Count |
| --- | ---: |
| Positive | 20 |
| Null | 6 |
| Invalid | 9 |
| Stopped | 2 |
| Killed | 8 |
| No-go | 2 |
| Inconclusive | 2 |
| Known theory | 3 |
| Mixed | 2 |
| Not run | 3 |
| Open | 1 |

The orthogonal result-kind projection reported 10 scientific results, 10
software qualifications, 9 invalid experiments, 6 null results, 2 stopped
work items, 8 killed directions, and 2 apparatus no-go outcomes. Human
acceptance was not inferred from any of them: `accepted_standing_changes` was
zero.

## Required supervision cases

- H-111 remains `OPEN` in the manual register, while the exact retrospective
  proposes `KILLED AS A THIN CROSS-DOMAIN EXECUTABLE COMPILER`; the audit
  interface survives. The view reports the conflict and does not edit it.
- H-112 remains `OPEN` in the manual register, while the completed analysis
  proposes `PARTIAL SUPPORT; NOT CONFIRMATION`. The result is not promoted to a
  causal or confirmed claim.
- The Erdős 686 task is `STOPPED BY STRATEGIC DECISION`; it remains distinct
  from the positive historical Erdős 686 partial result, a scientific null,
  and a kill of the problem.
- `T4b / Orion` remains marked active in the manual ledger but had no matching
  live task or callback in the bounded task snapshot. The view derives
  `missing_callback`.
- The live I6-ARC task was fresh and active at the snapshot instant, so it was
  not mislabeled stalled.

The chosen next action was to have the program owner adjudicate the H-111 and
H-112 register/ledger proposals and close or rebind the T4b/Orion callback
before authorizing another experiment.

## Field-by-field manual comparison

- The 58 structured campaign rows matched all 58 manual experiment-ledger rows
  on date, question, design/baseline, result, scientific disposition, and
  evidence pointer: 348 field comparisons, zero mismatches, zero missing rows.
- The hypothesis register contained 27 rows. Two exact state conflicts were
  reported: H-111 and H-112. They were emitted only as proposed changes with
  `authority_effect: none`, `standing_effect: none`, and `applied: false`.
- The manual research-episode index contains 22 curated strategic episodes,
  while the experiment ledger contains the 58-campaign denominator. The view
  reports `different_granularity`; it does not silently replace either count.

## Evidence discrepancies retained

Fifty-one of 58 primary evidence identities verified at the current local
paths. Seven remained visible as gaps:

- unresolved primary evidence for `DTS-SEARCH-1`, `STATE-LIFT-v2`,
  `DOCKER-FIVE-RESULTS`, `RESET-1`, and `AI4SCI-1`;
- the current `MEMORY.md` bytes for `SUPER-STAGE-0` differ from the retained
  reconstruction digest; and
- `FOUNDRY-0` points to an available directory without an exact repository
  head/tree identity.

No episode was dropped because its evidence was missing or changed.

## Adversarial coverage

Focused tests cover a summary that misleadingly says accepted while the typed
status is invalid, a changed declared source whose rows remain visible, missing
episode evidence, duplicate episode identities, overdue callbacks,
invalid-versus-null separation, and an unauthorized acceptance proposal. The
last is rejected while other proposals remain visible.

## Usability and remaining blocker

This is routine-useful for inspection: one command replaces manual traversal
of six source files, the 58-row structured denominator, and a bounded task
snapshot, while retaining disagreements instead of overwriting them.

It is not yet continuous. The smallest remaining step is a Workbench-owned
export that refreshes the compact task/callback observations and exact source
digests whenever a task changes or completes, then reruns this same read-only
projection. That export must still propose ledger/register changes; it must not
apply them or acquire Vela repository authority. No monitor, scheduler,
transcript store, new protocol object, or website is needed for that step.

Current friction: the first input requires manually capturing task state and
classifying the proposed H-111/H-112 updates. The 58 episode rows themselves
did not need to be recoded because the existing H-112 denominator was reused.
