# Independent review: maintained evidence-infrastructure consolidation

## Verdict

**BLOCKED** for producer commit
`27d368e6fbb111c1c65a51850e6da43596eabd50`, tree
`f47fc77de7aebbebe43abbbbad682681c0232005`, over base
`4685462c44b1f073870f31025ae73d1d8770ce73`.

The placement and non-authoritative scope are appropriate, the Protocol 1
surface is byte-identical, and the existing broad conformance union passes.
The maintained gate itself is not fail closed, however. The exact CI command
fails from a fresh locked Python 3.13 environment, and independent adversarial
bundles obtained `qualified_hold` despite stale or malformed runner evidence,
same-file account fixtures, an internal symlink alias, a forged permit,
invalid capture schemas and time, a reversed event lifecycle, and
comment-only Dockerfile controls. The OCI reader also accepted a manifest that
declared a missing layer.

This review changes no producer byte, releases no permit, calls no provider,
opens no protected adjudication, and has no Repository authority, Decision,
Standing, or scientific-record effect.

## Exact binding

- Producer ref:
  `origin/codex/evidence-harness-consolidation`
- Producer commit/tree:
  `27d368e6fbb111c1c65a51850e6da43596eabd50` /
  `f47fc77de7aebbebe43abbbbad682681c0232005`
- Parent/base commit/tree:
  `4685462c44b1f073870f31025ae73d1d8770ce73` /
  `13c5e0cf2e64be907cee4c0fd740ab0027118e13`
- Refreshed `origin/main`: the same base commit and tree
- Reviewed range:
  `4685462c44b1f073870f31025ae73d1d8770ce73..27d368e6fbb111c1c65a51850e6da43596eabd50`
- Delta: 6 files, 1,934 insertions, 0 deletions
- Protocol 1 root:
  `sha256:6ca327d6ec6f56c051e236be9eee42629e1f67dce393c1518e051ff8c14b279e`
- Reviewed at: `2026-08-22T03:24:05Z`

The handoff listed four paths. The immutable commit actually changes six; the
two unlisted package entry points were also reviewed:

- `.github/workflows/conformance.yml`
- `tools/evidence_qualification/README.md`
- `tools/evidence_qualification/__init__.py`
- `tools/evidence_qualification/__main__.py`
- `tools/evidence_qualification/qualification.py`
- `tools/evidence_qualification/test_qualification.py`

The SHA-256 digests of those exact producer files are, respectively:
`d5105db94a1dbab3ae93874b12c3c2d835dd2d6ca4727efc05532a91309913c2`,
`d2833c6d98171c8be01122311e94b4534ce6ae9c5e788dc3f4bf595c448abfb2`,
`a550d7c04ae6d8f89a9467836365f338b8406a98313bfd602bfa81c8d7ca08ac`,
`e51f321078d8b61dc8037a4513fa8b4030c9f21cd8563f926a4e4fe629002553`,
`f2db76664c7136c9b6ccf5140e69f693cafcf9dbbd5549dc649a8d6b378ec8fc`,
and
`dd1db0d18399d1ac77492ed68eb736e2b53ffa4508b6d2d6f08aeec27394da35`.

## Blocking findings

### EQ-01: self-verification escapes the locked Python environment

`_validate_self_verification` binds
`Path(sys.executable).resolve()`. In a virtual environment this resolves the
environment's Python symlink to the base interpreter. The producer test then
executes that base interpreter directly, outside the locked environment where
`jsonschema` was installed.

From the fresh clone, the exact workflow command failed:

```text
uv run --project conformance --locked python -m unittest \
  tools.evidence_qualification.test_qualification

FAILED (failures=1)
ModuleNotFoundError: No module named 'jsonschema'
```

Available-minor results were:

- Python 3.11: 20/20 passed because that base interpreter happened to have
  `jsonschema` globally;
- Python 3.12: 19/20, self-check failed;
- Python 3.13: 19/20, self-check failed;
- Python 3.14: 20/20 because that base interpreter happened to have
  `jsonschema` globally.

The mixed result is itself evidence that the self-check is host-state
dependent. It cannot establish that the documented current command works from
the current locked artifact.

Minimal correction: retain the virtual-environment executable identity instead
of resolving through it, or bind an exact locked `uv run --project
conformance --locked ...` invocation. Test the exact documented invocation
from isolated 3.11-3.14 environments with no globally installed dependency.

### EQ-02: runtime and reproducibility evidence can be syntactically forged

Independent mutations returned `qualified_hold` in all of these cases:

- `runner_version: false`, because the receipt field is never type-checked or
  bound to an expected runner version
  (qualification root
  `sha256:efa030743e169fa207f36c3d7559b95e3e634b222fbdc167f4a9cfc8199fab73`);
- the same account fixture path supplied twice as the claimed cross-day pair
  (root
  `sha256:961252c719fbb7ed308d408ac69ccfd079025b660c990d73b213034a4058c6db`);
- a Dockerfile with `ARG SOURCE_DATE_EPOCH` and `RUN --network=none`
  present only in comments
  (root
  `sha256:6ec676ea32f3dd45d1b89e2a21a936d0863c58579796a757c10e13dd1250a6e5`).

Separately, `_oci_identity` accepted an archive whose manifest declared a
layer digest and size but whose archive omitted that layer. It returned a
manifest/config/platform identity instead of
`oci_archive_invalid`. The current reader checks only index, manifest, and
config blobs; it does not establish a complete OCI archive.

Minimal correction: make the expected runner version a closed configuration
field and require exact receipt equality; require distinct account paths,
distinct original bytes, and distinct numeric source days; parse or anchor
effective Dockerfile instructions rather than matching comments; and validate
the complete OCI layout, unique safe member names, descriptor sizes, and every
referenced config/layer blob by digest. Add the exact adversaries above.

Builder independence may remain explicitly attested rather than performed by
this command, but the command must not upgrade incomplete bytes or arbitrary
labels into a passed gate.

### EQ-03: path and permit custody are not closed

A bundle-local directory symlink used as an intermediate path component was
accepted for all three schema files and returned `qualified_hold`
(`sha256:d4d93928879f94cf1813735714b3050a6bc92417b9026cfe5455d85765c70c61`).
`safe_relative` rejects only a symlink at the final path and therefore permits
aliases through parent components.

A participant permit with a forged schema, an unknown `forged` key, and a
boolean `attempt` also returned `qualified_hold`
(`sha256:790d53c68f53d05476770fdcb236963c97f2c37be026b645bfba296114f74402`).
The participant and neutral permits are not validated against a closed schema;
only a few values are selected with `.get()`.

Leaf symlinks, traversal outside the bundle, wrong hold status, pre-existing
consumption, and concurrent replay do block, and the atomic link/unlink race
had one winner. Those passing cases do not close the two bypasses above.

Minimal correction: reject a symlink at every component using descriptor-
relative no-follow traversal, and define exact schemas, fields, types, and
identity bindings for hold, template, held, authorized, consumed, launch, and
permit objects. Validate immutable permit bytes before one-shot consumption
and cover forged fields, boolean counts, schema drift, and parent aliases.

### EQ-04: capture lifecycle fields are retained but not validated

One adversarial capture simultaneously used forged launch, terminal, and
teardown schema values, `completed_at: -1`, and reordered
`turn.completed` before `thread.started` and `turn.started`. After
honestly recomputing the byte receipts and capture manifest, the bundle still
returned `qualified_hold` at
`sha256:a7c64cc46d0ef53cedbacba69e6ad02a5c9215634f8a0d7c15760c2c186999ec`.

The implementation requires event counts but not order, and it includes
`schema` and `completed_at` in exact key sets without checking their values
or types. Thus complete byte custody is present, but a complete valid launch /
event / terminal / teardown lifecycle is not established.

Minimal correction: require the exact schema constants; validate run identity,
status, and RFC 3339 timestamps with nonnegative monotone ordering; and use a
closed lifecycle state machine that rejects out-of-order, duplicate, missing,
and unknown lifecycle events. Preserve raw event bytes and canonicalize only
the derived response ordering.

## Boundaries that passed

- **Architecture and placement:** this is one neutral top-level
  `tools/evidence_qualification` qualifier. Its main path invokes no
  provider, controller, scheduler, runner, scientific scorer, Vela object
  writer, authority signer, Decision, or Standing transition. The source-owned
  runner remains responsible for actual provider and process execution.
- **No normative widening:** `conformance/`, `schemas/`, Rust crates,
  Protocol documentation, paper evidence, and current evidence claims are
  byte-identical across the producer range. The conformance tree inventory is
  identical, and Protocol 1 remains exactly
  `sha256:6ca327d6ec6f56c051e236be9eee42629e1f67dce393c1518e051ff8c14b279e`.
  The new workflow step runs non-Protocol tooling tests only.
- **Provider schema boundary:** only deletion of a present
  `uniqueItems: true` keyword is allowlisted; missing, duplicate, unsupported,
  or non-`true` deletion requests block. Both schemas pass Draft 2020-12
  meta-validation, the full registered schema remains the local validity
  authority, and both raw schema digests enter the receipt.
- **Response handling:** malformed, missing, duplicate, and unknown closed-set
  identities block; valid response-order variants retain distinct raw bytes
  and derive the same canonical response root.
- **Decimal and token behavior:** Decimal-only half-even serialization was
  byte-identical on 3.11-3.14; binary floats and boolean/negative token counts
  block. Large nonnegative input/cached-input telemetry remains telemetry,
  while the output-token ceiling and full response schema still fail closed.
- **Pre-key buffer behavior:** the helper reads snapshot entries into immutable
  byte buffers, rejects leaf symlinks and manifest drift, retains the original
  buffers after post-read mutation, and rejects a later reread. EQ-03 still
  applies to parent-component aliases.
- **Workflow posture:** workflow permissions remain `contents: read`; actions
  and uv are commit/version pinned; Python dependencies are locked; the offline
  Zizmor audit reports no finding. EQ-01 means the newly added exact test step
  is not presently green from a clean locked 3.13 environment.
- **Math ownership:** refreshed `vela-science/math` `origin/main` is
  `cf6d76687b205a39e2515e9fec7087c819454d2f`, tree
  `f8e9e8d3b99226ed6bba62026396d5f17ea9351e`. Its
  `tools/result_runner/next_campaign_v1` owns Lean/lake/compiler,
  source-verification, evaluator, runtime, and campaign-permit behavior.
  Vela's new qualifier adds none of that domain implementation.

## Historical evidence boundary

The historical producer commits remain exactly reachable:

- sealed 16-cell result:
  `3207066f22f09b578f354b7028f55559e7b45926`;
- schema-stopped state:
  `d3bff9206609c53a0dc9b2ef7f85bbdc894a9904`;
- order-contract stopped state:
  `f14616e341929e7ad74927a846cba12e5889154e`;
- complete 36-cell capture:
  `5694bebac03b062d6acdce5a2a900551850e6a1c`;
- scored result:
  `4524c8f776943a267e04e03e9a237ecaed14bc2c`.

The consolidation edits none of those artifacts or their independent review
lineage. The scored result remains Git/documents 12/12, neutral wrapper 12/12,
Vela 11/12 with one authority error; every positive gate is false and
`positive_gate` remains `not_supported`. It remains a fixed synthetic
internal negative result with no scientific acceptance, external replication,
broad productivity, Protocol/Core, Repository authority, Standing, or
Decision claim.

## Independent checks

Passed from the fresh detached clone:

```text
uv run --project conformance --locked ruff check tools/evidence_qualification
uv run --project conformance --locked zizmor --offline --min-severity medium .
PYTHONDONTWRITEBYTECODE=1 uv run --project conformance --locked ./conformance/check-core.sh
PYTHONDONTWRITEBYTECODE=1 uv run --project conformance --locked python conformance/verify_protocol_1.py
git diff --check 4685462c44b1f073870f31025ae73d1d8770ce73..27d368e6fbb111c1c65a51850e6da43596eabd50
```

`check-core.sh` completed successfully and ended
`core surface: ok (external Lean not selected)`. Protocol verification
reported 77 normative and 39 informative files at the root above.

Failed:

```text
uv run --project conformance --locked python -m unittest \
  tools.evidence_qualification.test_qualification
```

The failure is EQ-01. The additional adversarial qualifying bundles establish
EQ-02 through EQ-04.

## Residual boundaries

Even after correction, the qualifier should state these limits exactly:

- builder independence is attested by receipts rather than independently built
  by this command;
- the qualifier deliberately invokes no provider;
- a future provider incompatibility requires an allowlisted maintained code
  and test change;
- historical artifacts do not import this qualifier and retain their own
  immutable roots;
- qualification is non-authoritative evidence and cannot release a benchmark
  permit, bypass a source-owned runner, or create a Vela Decision or Standing.
