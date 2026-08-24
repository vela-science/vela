# Independent executable reducers

This Phase III P1.2 artifact implements the accepted minimal Standing model
three times: in Rust, Python, and JavaScript. Each executable parses, validates,
and reduces the same frozen histories without importing, invoking, or generating
another implementation. The comparison harness only launches processes,
checks bytes and hashes, and compares declared expectations; it contains no
replay logic.

The versioned input described in [FORMAT.md](FORMAT.md) is a proof-artifact
interchange format. It is not a Vela Protocol schema, wire alias, CLI surface,
compatibility promise, or reusable product framework. Nothing here is linked
into a Vela runtime crate.

## Frozen evidence

The corpus has 15 cases: 14 format-valid complete replays and one structural
`invalid_format` case:

- a fresh accepted correction and its stale-root twin;
- unauthorized, wrong-Repository, misattributed, stale-read-set, ineligible,
  and invalid-correction-reference Decisions;
- suffix-continuation variants that place every semantic rejection class
  before a later admitted Decision;
- a two-rejection history that preserves observation order and then admits a
  valid suffix;
- a syntactically invalid duplicate Decision id;
- Submission and Verification records that leave Standing `unassessed`;
- two local authorities binding the same Submission and Verification records;
  one accepts locally and the other records a rejecting Decision locally; and
- the fresh correction with and without a descriptive dependency edge.

Invalid correction ordering supplies the event-order failure. Decision-id
uniqueness is a proof-interchange well-formedness constraint checked before
replay, not an additional Decision admission predicate in the Lean model.

For every case, the harness runs each reducer twice, requires byte-identical
repetition, then cross-compares all three implementations byte-for-byte. That
is 90 reducer invocations per harness run. Format-valid histories emit the same
canonical compact result after all records: each rejected Decision is a state
no-op recorded by zero-based index and stable code, and later records continue.
Only structural parse or validation failure terminates without replay.

The frozen corpus aggregate is:

```text
5aabd16a164b78e978225dad1f32f170fbafac62abcab54c7269fb1f74bc4cc2
```

It is SHA-256 over canonical JSON containing each case id, input SHA-256, and
output SHA-256 in manifest order. `corpus/manifest.json` binds those hashes to
the committed inputs and agreed outputs. Regenerating cases is deterministic;
`--freeze` is the explicit maintainer operation that rewrites agreed outputs
and updates their binding.

## Check

The checked toolchain identities are:

```text
rustc 1.97.1 (8bab26f4f 2026-07-14)
cargo 1.97.1 (c980f4866 2026-06-30)
Python 3.13.9
ruff 0.16.0
Node.js v25.9.0
Lean 4.19.0 (the adjacent accepted P1.1 package)
```

From this directory:

```bash
python3 corpus/generate.py
(cd rust && cargo build --locked)
python3 harness/verify.py --freeze
python3 harness/verify.py

(cd rust && cargo fmt -- --check)
(cd rust && cargo clippy --locked -- -D warnings)
python3 -m py_compile corpus/generate.py python/reducer.py harness/verify.py
node --check javascript/reducer.mjs
(cd ../lean && lake build)
```

After the frozen corpus is committed, ordinary verification omits the first
three regeneration/freezing commands and runs `python3 harness/verify.py`.

## Lean comparison and semantic boundary

The manifest declares only the reviewed finite Lean results; no reducer is an
oracle for another. The harness independently checks:

- fresh: predecessor `superseded`, replacement `accepted`, dependent
  `accepted`, with the separate projection reporting `needs_reassessment`;
- stale: predecessor `accepted`, replacement `unassessed`, dependent
  `accepted`; and
- plural authority: the accepting Repository sees `accepted`, while the other
  Repository remains `unassessed` for the same source records.

Changing the descriptive dependency edge is also checked to change only the
non-authoritative `reassessment` projection. It cannot affect admission,
Events, root, or canonical Standing.

Agreement among three small implementations is evidence that the reviewed
semantics are not an incidental property of one language or evaluator. It is
not evidence of universal scientific truth, productivity, completeness,
security of the shipping Vela implementation, or suitability as a public API.
