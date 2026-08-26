# Vela conformance

This directory contains the small, implementation-independent corpus for the
Vela Protocol 1 release-candidate boundary. `protocol-1.json` is the
machine-readable map from the specification to exact schemas, fixtures,
vectors, independent implementations, and non-normative examples;
`verify_protocol_1.py` checks its paths and SHA-256 bindings.

Run all checks:

```bash
uv sync --project conformance --locked --all-groups
uv run --project conformance --locked ./conformance/check-core.sh
```

The independent Python reader supports Python 3.11 through 3.14; the independent
JavaScript canonical reader uses the Node runtime already required by the
current-object emitter. CI uses an exact Python interpreter while
`requires-python` keeps that reader contract independent of a single minor
line. CI-affecting Python tools are part of the same lock:

```bash
uv run --project conformance --locked ruff check conformance
uv run --project conformance --locked zizmor --offline --min-severity medium .
```

The corpus protects seven contract families:

1. canonical JSON bytes and SHA-256 roots in Rust, Python, and JavaScript;
2. byte-identical Submission and Verification emission from an independent
   JavaScript implementation;
3. checked JSON Schema 2020-12 descriptions and frozen current-object fixture
   roots;
4. a non-authoritative correction-impact projection, including independent
   support-route survival and bounded fail-closed diagnostics; and
5. verification of the current six-record, eleven-Event Math authority chain
   from an explicit external sequence-one anchor, including thirteen
   fail-closed mutations and both signed correction transitions;
6. byte-identical independent Python and JavaScript object reading, including
   canonical bytes, DSSE type, Ed25519 signature, full root, handle, and signer;
   and
7. four executable reference flows whose authority effect remains explicitly
   none.

`current-objects/` contains deterministic signed Submission and Verification
vectors. The seed files are public fixture material only. They are never used
as production identities. `manifest.json` freezes the exact fixture bytes;
the schemas document structure and carry no authority or Standing effect.

`fixtures/correction/diamond-input.json`,
`diamond-expected.json`, and `diamond-adversarial.json` are synthetic
conformance vectors only. They let the Rust reducer in `vela-cli` and the
clean-room Python reader agree on exact bytes before a real correction fixture
exists. They earn no scientific or protocol-breakthrough credit.

`fixtures/authority/math-coh-00/` retains public signed history and no private
key. Its Python reader independently verifies the exact chain, the Erdős 321
and 94 predecessor-to-correction transitions, and the three-Claim terminal
state without Vela, Rust, Git, or network access. The explicit trust anchor is a separate
verifier input; this vector does not claim that current CLI read paths load the
local trust pin or that the production history verifier checks every
fixture-level cross-link.

Historical reducer cascades, AcceptancePolicy experiments, actor-registration
previews, and their duplicate Python/TypeScript readers remain available in
Git history. They are not current runtime contracts and are intentionally
absent from this corpus.

## Semantic scenario index

The table maps Protocol 1 behavior to executable evidence. `PASS` means the
test reproduced the expected state or refusal. It does not mean scientific
acceptance or release readiness.

| Scenario | Executable evidence | Required result |
| --- | --- | --- |
| Submission | `review_acceptance`; `genesis` | Authenticated Submission, Claim, Artifact, and pending Proposal are retained; no Event or accepted Standing is created. |
| Passing Verification | `review_acceptance`; `genesis` | The scoped record is retained and changes no Standing before a Decision. |
| Failed, contradictory, or incomplete Verification | `disposable_rejection_lifecycle` | Evidence remains visible; acceptance is unavailable and accepted Standing does not change. |
| Unauthorized Decision | `review_acceptance`; `genesis` | Missing signer, authority, or trust selection refuses without Repository, Event-log, or Standing mutation. |
| Authorized accept and reject | `review_acceptance`; `disposable_rejection_lifecycle` | Accept admits linked review and Claim Events; reject closes the Proposal while accepted Standing stays unchanged. |
| Correction and supersession | `correction_impact`; `authority_chain_interop`; `repository_decision::tests::revise_replaces_exactly_one_predecessor` | The exact predecessor is retired, the successor may enter Standing, and predecessor history remains replayable. |
| Retraction and Proposal withdrawal | `repository_decision::tests::withdrawal_accepts_only_the_exact_accepted_claim`; `claim_standing::tests::an_accepted_withdrawal_retracts_rather_than_accepts`; `wording_contract` | Authorized retraction removes only the exact accepted Claim; producer withdrawal closes only its pending Proposal and emits no Event. |
| Rejected-history preservation | `disposable_rejection_lifecycle` | Submission, Proposal, Verification Records, attributed Decision, and rejection Event remain addressable. |
| Clean-clone replay | `genesis`; `portable_divergence`; [`examples/neutral-replay/check.sh`](../examples/neutral-replay/check.sh) | Complete governed bytes reproduce the accepted set, Repository root, authority Event-log root, and Standing. Replay does not execute a scientific Method. |
| Missing or corrupt Artifact | `genesis`; [`examples/neutral-replay/check.sh`](../examples/neutral-replay/check.sh) | Strict replay fails closed and emits no partial Standing. |
| Missing, malformed, mismatched, or environment-supplied trust pin | `genesis`; `authority_chain_interop` | Every governed read requires the independently selected sequence-one root and ignores `HOME`. |
| Changed Method binding | `review_method_check`; `genesis` | Changed profile, property, actor, nonclaim, or retained Method bytes refuse; source-owned Method execution remains outside replay. |
| Canonical bytes, schemas, and cross-reader roots | `canonical_hashing_conformance`; `wire_schemas`; `verify.py` | Rust, Python, and JavaScript agree on exact bytes and roots and reject the negative vectors. |
| Portable input under local authority | `portable_divergence` | The same authenticated Submission can receive different local Decisions and Standing without global consensus. |

Run the focused Rust targets named above with `cargo test --locked -p
vela-cli --features test-support --test <target>` or, for protocol targets,
`cargo test --locked -p vela-protocol --test <target>`. Run the unit paths with
their full names. The full portable corpus remains:

```bash
uv run --project conformance --locked python conformance/verify.py
```
