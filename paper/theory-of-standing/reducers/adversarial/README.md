# Deterministic adversarial model corpus

This Phase III P1.3 artifact grows the accepted minimal model without growing
its interchange format. `generate.py` is an ordered, bounded set of reviewable
mutation templates over `theory-of-standing.proof-history.v1`. It has no random
input, seed, Vela wire object, cryptography, environment model, or Core replay
concept. `verify.py` invokes the three independent P1.2 reducers and contains no
replay implementation.

The generator emits 34 format-valid histories. Their rejection-observation
distribution is 14 with zero, 19 with one, and one with two. The exact
overlapping class distribution is bound in `manifest.json`; the primary
mutation counts are:

| Class | Histories |
| --- | ---: |
| valid suffix after a semantic rejection | 20 |
| ineligible action | 6 |
| unauthorized performer position | 3 |
| stale root | 4 |
| stale read set | 3 |
| invalid correction reference | 2 |
| wrong Repository / misattributed | 1 / 1 |
| descriptive dependency mutation | 3 |
| plural authority over the same source records | 2 |
| record-order mutation | 4 |

The P1.2 frozen corpus remains the small named regression layer, including its
structural `invalid_format` duplicate-Decision-id witness. P1.3 does not copy
those inputs or commit generated per-case outputs. Its manifest instead binds
the generator identity and parameters, class counts, ordered input and output
aggregate hashes, reducer source hashes, 16 sampled case ids, and links to five
named P1.2 witnesses.

## Checks

From `paper/theory-of-standing/reducers` after building the Rust reducer:

```bash
(cd rust && cargo build --locked)
python3 adversarial/verify.py --freeze
python3 adversarial/verify.py
python3 adversarial/verify.py
python3 adversarial/verify.py --case stale_root_lower_suffix
```

Each full run executes all 34 histories once in each of Rust, Python, and
JavaScript: 102 reducer invocations. Two complete runs therefore make 204
invocations and must reproduce the same manifest and summary. On a failure,
the assertion includes `case_id`; the final command reproduces any named case.

The checked identities were Rust 1.97.1 (`rustc` commit `8bab26f4f`), Cargo
1.97.1 (`c980f4866`), Python 3.14.4, ruff 0.5.4, Node.js v25.9.0, and Lean
4.19.0 (`6caaee842e94`). The locked conformance project selected CPython 3.13.9.

The final deterministic bindings are:

```text
input aggregate   759a37371786040a1dbb8208f7f1e394d2e5dde1c9efe81152dc6f5fa46f6403
output aggregate  9c493a046bdea61e8076a485f8e5f18f49443a2c75b89a2af8925c0631e65983
combined aggregate 20bb27b76154e8802f81596dc24587d407a46896ccb15ff62f519d36d97e23c1
two-run stdout    3221c47268933360c802edf9b7f152aad2ff61878d9e676edd2e1a0214fc7735
manifest bytes    ba4090caa65476201454a1608fc1cf10e4498bb0c34d348741e39b3b301bcae2
```

The verifier requires canonical byte-identical output across all three
implementations and checks declarative expected roots, Standing, Events, stable
indexed rejection observations, and derived reassessment. Mutation sentinels
also require:

- continuation after all seven semantic rejection codes;
- the rejected Decision to leave the root and Standing needed by a valid
  suffix unchanged;
- every admitted abstract Event to retain exactly the source Decision id,
  Repository, authority label, performer, and action;
- correction-reference failures to remain rejected;
- multiple rejection observations to preserve record order; and
- descriptive dependencies to change only the separate non-authoritative
  reassessment projection.

`AdversarialSamples.lean` independently checks 16 stratified generated cases
through 14 kernel-checked theorem groups. The selection includes every
transition and semantic failure class, plural authority, and both sides of the
dependency projection comparison. The Lean definitions are the comparison
surface; none of the reducers is used as an oracle.

See [COVERAGE.md](COVERAGE.md) for the separate current-Core/wire inventory.

## Scope and nonclaims

This is implementation-neutral evidence about the reviewed small model. It is
not a Vela Protocol schema, shadow wire format, shipping API, Core security
certification, universal scientific-truth claim, or productivity result. It
does not model incomplete retained slices, canonical object roots, signatures,
DSSE envelopes, durable repository identity, or execution environments; those
remain owned and tested at current Core/wire boundaries.
