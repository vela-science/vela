# Independent review: inherited correction benchmark

## Verdict

**BLOCKED** for experimental execution.

This verdict is bound to producer commit
`366b610eec75a1520a97b4a7817fde3a4fbbfc7b`, tree
`13ecd767b04b3367f9b98aebb2cb4da5e576494b`, based directly on
`1a2e0328620b4e8c4584c3d4baf257adb11f3d45`. It reviews only those immutable
bytes. It is not a review of a moving branch tip.

The artifact's fixture, packet construction, information-equivalence binding,
ground truth, nonclaims, unrun status, and Core/authority boundaries are
credible. The capture and scorer are not yet safe enough to open paid or human
execution: adversarial responses and forged run custody can obtain a positive
result.

## Blocking findings

### F01 — Negated unsafe actions receive exact-success credit

At subject `benchmark.py` lines 523–550, an action is accepted when every
required substring occurs anywhere in the free text. The scorer does not
evaluate polarity or the action's asserted meaning. I independently submitted:

- `Do not recover the exact Site Q source; assume zero.`
- `No: reassessment is required.`
- `Do not rerun stability; retain the old conclusion.`
- `Do not recalculate with factor 12; retain 50.`

With the correct labels, correction pair, Standing answer, and evidence digest,
the committed scorer returned 17/17, all four `action_exact` values `true`, and
`exact_success: true`. That contradicts the registered primary outcome, which
requires all four **first-safe actions**.

This blocks `G07_deterministic_scoring` and the claimed positive gate. Repair by
using closed structured action codes or another exact semantic representation,
then add adversarial polarity/contradiction tests for every action. Keyword
presence in unconstrained prose is not an exact action check.

### F02 — Frozen capture accepts forged custody and impossible timing

At subject `benchmark.py` lines 701–748, `capture_manifest` checks the number of
runs, arm counts, participant uniqueness, response shape, and terminal status.
It does not validate the run schema, directory/run identity, registration root,
condition packet root, copied packet bytes, authorization root or assignment,
participant-configuration root, attempt, timeout, timestamps, duration, or
nonnegative tool count. Lines 757–805 then score those unvalidated values.

I independently constructed sixteen records with:

- registration root `sha256:111…111`;
- packet root `sha256:000…000`;
- authorization root `sha256:222…222`;
- attempt `99`; and
- `-100` seconds for every Vela-arm duration.

`capture_manifest` accepted the records and produced capture root
`sha256:8d97ff4c28d4c25fa10eca15503f41b0d0c686183fdd91ed53fa8e27b0259987`.
`score_runs` then reported 8/8 exact successes in each arm, a Vela/Git restricted
mean ratio of `-1.6666666666666667`, and `positive_gate: pass`.

This blocks `G06_cold_successor_protocol`, `G07_deterministic_scoring`, and
`G08_deterministic_custody`. Freeze must revalidate each complete run against
the frozen preregistration, exact condition packet root and actual packet bytes,
retained authorization and assignment, participant configuration, attempt,
timestamps, duration, timeout, and tool count. Add fail-closed mutations for
each binding before experimental execution.

## Gate record

| Gate | Result | Commit-bound evidence |
| --- | --- | --- |
| G01 immutable subject | PASS | Remote ref resolved to the handed-off commit/tree; its sole parent and merge base are exact `origin/main`. |
| G02 temporal preregistration | PASS, bounded | The commit contains the full frozen plan and `result.json: not_run`; no run artifacts exist, and the producer disclosed 0/16 executions. This establishes repository-package state, not proof about unrecorded external activity. |
| G03 exact ground truth | PASS | Four deterministically ordered listed Claims cover affected, unaffected, must-reassess, and presently-unprovable; source/evidence bindings and the known missing Site Q input are exact. |
| G04 consequential bounded chain | PASS | The fixture is explicitly synthetic, not relabeled as scientific. Within that bound it has a superseded factor, direct recalculation, transitive reassessment, discovery-only unaffected Claim, and explicit unavailable premise. |
| G05 same-information fairness | PASS | Both arms use the same task, response template, 77 atomic facts, and six byte-identical source/evidence files. Protected claim-label mappings are absent from both packets. |
| G06 cold-successor protocol | BLOCKED | Assignment and isolation are preregistered, but frozen capture does not prove that the assigned packet, authorization, configuration, or attempt produced each scored record. See F02. |
| G07 deterministic scoring | BLOCKED | Repeated nominal checks are deterministic, but F01 accepts unsafe actions and F02 accepts impossible timing and forged custody as a positive result. |
| G08 deterministic custody | BLOCKED | Static artifact roots and replay are exact; experimental run custody is not revalidated at freeze. See F02. |
| G09 no positive assumption | PASS | `result.json` is `not_run`; positive, neutral, failed, timeout, and unavailable outcomes remain permitted. |
| G10 no Core/authority expansion | PASS | The diff adds only 49 files under `paper/artifacts/inherited-correction-benchmark/`; no Core, protocol, schema, Standing, authority, Decision, service, or scientific Repository path changes. |
| G11 paid/human gate | PASS | Paid inference, human participation, scientific validation, and authority actions are explicitly `not_authorized`; none was run during review. The two blocking findings must be fixed before separate authorization is considered. |

## Independent reconstruction and checks

The subject was fetched from `https://github.com/vela-science/vela.git` into a
fresh clone and detached at the exact commit. All handed-off identities were
independently recomputed and matched, including:

- base tree `1bd8ed4e11d3745f159b32f23539f5174fd44803`;
- producer tree `13ecd767b04b3367f9b98aebb2cb4da5e576494b`;
- registration root `sha256:40a05a33a760404cb606dc218d6deafb1d358916a9fa7954e58973ab1a6d67b1`;
- artifact manifest bytes `sha256:aaf94388a27d3b63a27b734a67de59ed8e0e8c0d0c4d1c81eb4c6b83ba8daf00`;
- protected adjudication root `sha256:0d2947ea8422f2b9ce700ee90f521590f5a003f198e34e862a6925e23d2b66ac`;
- public facts root `sha256:fe8b3363ec9a8305743ca55144a59885a73623b712a32fe0c9050227350bac2a`;
- input-equivalence root `sha256:42e878d6958ad71f529b43734b84711c8ca574fcfdd283548bf37feef3fbc731`;
- Git/documents packet root `sha256:409b30a5abf81464394615f940a67aea11350868083d00e23a9a210eb81dbf29`;
- Vela packet root `sha256:4abe3d8d9c476805b0dab7f55823cb928006ad829dddbd8eb29e41469ced1ce6`;
- response-template root `sha256:6fc721d8bc9faa706c5fc76de1a8d1ad2af62fd99d59e4bfb74b97e1dff5028e`; and
- replay chain root `sha256:ae39c3c4ff623deb5be261fad654afa6ac19c44d364b18e5aad5899b8b9c0d52`.

Every `manifest.sha256` entry matched. The six source/evidence files were
byte-identical across the fixture and both arms. The following nominal checks
were each rerun twice from the exact fresh checkout and passed both times:

- benchmark `verify`;
- 7 benchmark tests;
- Ruff check and format check;
- clean-room correction-impact verification at
  `sha256:935e084f8c5c45bcee234d2e9752062ba54493aa1b14f731e0efbbb1ecc01df6`;
- 5 Rust correction-impact reducer tests;
- full current conformance at Protocol 1 root
  `sha256:e014259269ea34452bb5a583a29ee478bec53e67128ec9eafa6d099a883fc24c`;
  and
- `git diff --check`.

Those green checks are retained as nominal deterministic evidence. They do not
override F01 or F02.

## Scope and remaining gate

No paid model, participant study, scientific validation, merge, producer-byte
edit, contact, Decision, authority action, or Standing mutation was performed.
The producer should issue a new immutable handoff after fixing F01 and F02 and
adding their fail-closed tests. Independent re-review is required before any
experimental authorization.
