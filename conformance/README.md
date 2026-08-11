# Vela conformance

This directory contains the small, implementation-independent corpus for
Vela's current public object boundary.

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

The corpus protects six contract families:

1. canonical JSON bytes and SHA-256 roots in Rust, Python, and JavaScript;
2. byte-identical Submission and Verification emission from an independent
   JavaScript implementation;
3. checked JSON Schema 2020-12 descriptions and frozen current-object fixture
   roots;
4. exact witness and bounded-Claim agreement;
5. a non-authoritative correction-impact projection, including independent
   support-route survival and bounded fail-closed diagnostics; and
6. verification of a retained four-record authority chain from an explicit
   external sequence-one anchor, including thirteen fail-closed mutations.

`current-objects/` contains deterministic signed Submission and Verification
vectors. The seed files are public fixture material only. They are never used
as production identities. `manifest.json` freezes the exact fixture bytes;
the schemas document structure and carry no authority or Standing effect.

`fixtures/exact-witness-floor.json` is a normative test vector.

`fixtures/correction/diamond-input.json`,
`diamond-expected.json`, and `diamond-adversarial.json` are synthetic
conformance vectors only. They let the Rust reader in `vela-edge` and the
clean-room Python reader agree on exact bytes before a real correction fixture
exists. They earn no scientific or protocol-breakthrough credit.

`fixtures/authority/math-0.972.1/` retains public signed history and no private
key. Its Python reader independently verifies the exact chain and terminal
state without Vela, Rust, Git, or network access. The explicit trust anchor is
a separate verifier input; this vector does not claim that current CLI read
paths load the local trust pin or that the production history verifier checks
every fixture-level cross-link.

Historical reducer cascades, AcceptancePolicy experiments, actor-registration
previews, and their duplicate Python/TypeScript readers remain available in
Git history. They are not current runtime contracts and are intentionally
absent from this corpus.
