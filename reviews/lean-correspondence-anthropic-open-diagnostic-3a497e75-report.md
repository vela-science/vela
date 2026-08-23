# Independent prelaunch review: Anthropic open diagnostic pilot

## Verdict

**BLOCKED** at producer commit
`3a497e75e85690a7bf03563e00d81fe0dbc339e5`, tree
`d64396fe72b04833d78a594e33576123b284fdba`.

The frozen design, source bindings, arm bytes, hold state, and claim ceiling
reproduce. The artifact is not execution- or scoring-ready because the exact
held permits are incompatible with the maintained qualification/custody
boundary and the scorer does not derive its booleans from immutable participant
evidence or an exact adjudication.

## Exact artifact binding

- artifact path:
  `paper/artifacts/lean-correspondence-anthropic-open-diagnostic-pilot`
- artifact root:
  `sha256:d453622eeb3afb1eaeef5b0aaf5e323194df352681221146d42901993c9bb110`
- assignment root:
  `sha256:cd53dcfcc0293b7ec2bb88297b146f8868ba34ea6b354867bde5a646cd3732d1`
- permit-set root:
  `sha256:352930210c092f4ac51d10c90512b24d945ca794ab2fec6609c03b099316db10`
- registration root:
  `sha256:9ce35870648a6eacf0f7b2c3970c7151227620c3d884aad54e7d30e208abcf96`
- registration-contract root:
  `sha256:c1793b548823dacfa2b471a0c4e52ac246440c4d330aa7e4500c3b9b75c7f324`
- hold-state root:
  `sha256:af4d1c9413d4939bfff5b6b2d0cc7e85b75f4a1e4363367002c73d85bee7f096`
- custody root:
  `sha256:6979783a9df63cf9614e77a17917566b5577fb65090ea59dada0efdaaa47913e`

## Reproduced passes

The review used a fresh clone from the hosted remote, detached at the exact
producer. `git fsck --full --strict` passed. The producer's deterministic
generator reproduced all 39 files and the artifact root with zero diff. The
artifact verifier passed with live commit/tree/report/result byte bindings;
all JSON parsed; diff hygiene passed; and the focused suite passed 23/23.

All six selected assignments are the exact Anthropic `configuration-b` rows
from the frozen Stage A package. Each prompt and packet is byte-identical to
its source assignment, and the response schema and case-selection bytes are
also identical. There are exactly three cases, two arms, one Anthropic
configuration, six distinct cells, six distinct participants, and one attempt
per cell. All six new permits are held and marked non-releasable; provider,
credential-content, response, terminal-capture, and scoring counts are zero.
The original 12 Stage A permits remain held at their exact permit-set root and
the producer diff is confined to the new additive artifact.

The Anthropic v4 calibration/amendment/review identities reproduce. The exact
36-cell negative result and independent review also reproduce: Git/documents
12/12, neutral wrapper 12/12, Vela 11/12 with one authority error, all positive
gates false, `positive_gate=not_supported`, and `authority_effect=none`. No
stale 16-cell positive wording is admitted.

The diagnostic gate is realizable and equality is not lift. It requires at
least one raw component miss, assisted noninferiority on all four components
within each case, zero assisted false authority/scientific inference, and a
strict aggregate component increment. A per-case reversal and an assisted
safety error fail.

The claim ceiling is proportionate: even a future PASS is Anthropic feasibility
on these exact open cases only. It cannot satisfy the two-provider Stage A,
G3, Phase 0, Stage B, cross-provider, scientific, human, breakthrough,
Frontiers, Protocol/Core, Repository authority, Decision, or Standing claims.

## Blocking findings

### AD-01: the held permits are not accepted by the maintained qualification boundary

Every new permit uses
`vela.lean-correspondence-anthropic-open-diagnostic-permit.v1`. The maintained
qualifier accepts only the closed
`vela.tooling.closed-launch-permit.v1` field set. Directly passing a new permit
to its `validate_permit` function fails with `permit_fields_invalid`.

The diagnostic permits omit maintained launch fields including `run_id`,
`condition`, `runner_version`, `runtime_source_root`, `image_digest`, exact
registered/provider schema roots, `issued_at`, and the maintained held-state
shape. The package also freezes no per-cell run-input materialization receipt,
provider-request root, offline validation receipt, or complete maintained
qualification receipt for the six exact prompt/packet/permit combinations.
Consequently a call cannot use the reviewed bytes without creating a new
permit and execution bundle after review.

Smallest repair: generate six exact maintained closed launch permits and six
exact run-input/provider-request derivatives from the reviewed prompts and
packets; bind their source, registered/provider-schema, runtime, image,
configuration, registration, and cell identities; run the maintained
qualification gate over the complete held execution bundle with zero calls;
freeze its receipt/root; and re-review the regenerated package before releasing
any permit. A bespoke-to-maintained permit conversion after PASS is not an
acceptable execution step unless the conversion and exact output bytes are
already frozen and reviewed.

### AD-02: the scorer can pass invented labels without participant evidence

`scorer.py` consumes six rows of caller-supplied booleans. It does not consume,
hash, or validate raw participant responses, response-schema receipts,
terminal/custody receipts, the complete capture root, or a fixed open
adjudication. A synthetic input with one raw relation boolean set false and
all other booleans true returns `diagnostic_gate_pass=true` even though this
prelaunch artifact contains zero responses or captures. The input therefore
asserts the adjudication result instead of deriving it.

The scoring contract also registers restricted-time and tool-call differences
as secondary estimands, but the scorer output does not compute or emit either
one.

Smallest repair: freeze an exact source-bound open adjudication for the three
cases; add a fail-closed capture-to-score compiler/scorer that consumes the
immutable six-cell custody root, validates every terminal/raw response against
the registered schema and assignment, derives all four component booleans from
the exact response plus adjudication, and retains failures/timeouts/malformed
responses in the denominator. Bind the scorer and adjudication roots in the
registration and permits. Emit the declared Decimal-canonical restricted-time
and exact tool-count secondary estimands. Add regressions proving that missing,
forged, cross-bound, duplicate, or response-free evidence cannot score, and
that only one score attempt over the sealed capture is accepted.

## Boundary

No producer byte was changed. No permit was released or consumed. No credential
was opened, no provider was called, no response was generated, and no scoring,
Stage B, Protocol/Core, authority, Decision, or Standing action occurred.

