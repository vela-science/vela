# Vela conformance vectors

This directory carries the portable test vectors for Vela's prelaunch `0.800`
protocol candidate. Passing them demonstrates agreement on the named byte and
reducer contracts. It does not certify scientific truth, policy authority, or
compatibility with future candidates.

## Reducer contract

Each `fixtures/cascade-fixture-*.json` contains:

- `genesis_findings`: the initial typed state;
- `event_log`: ordered canonical events; and
- `expected_states`: the reducer-owned projection after replay.

An implementation parses the fixture, replays the event log, projects the same
effect rows, and compares them with `expected_states`. The current set contains
14 `fixture_version: 6` fixtures. Its manifest records the byte length and
SHA-256 of every cascade fixture; the verifier refuses drift.

The source of truth for mutation kinds is
`crates/vela-protocol/src/kernel/reducer.rs`'s
`REDUCER_MUTATION_KINDS`. The Rust coverage test derives its obligation from
that constant, so adding a reducer arm without a fixture fails.

Reference implementations:

- Rust: `crates/vela-protocol/src/kernel/reducer.rs`
- Python: `clients/python/vela_reducer.py`
- TypeScript: `clients/typescript/vela_reducer.ts`

Run the focused checks:

```bash
cargo test -p vela-protocol --test cross_impl_reducer_fixtures
python3 conformance/verify.py
```

`verify.py` checks the manifest, runs the Python reducer and canonical-hashing
vectors, then runs the TypeScript reducer when Node supports native TypeScript.

## Other public vectors

- `gate-vectors.json` pins verifier-attachment gate outcomes.
- `canonical-hashing.json` pins `vela.canonical-json/v1` bytes and digests.
- `attempt-id.json` pins deterministic attempt identifiers.
- `decision-binding.json` pins decision preimages and their consumed roots.
- `spec-surface.v1.json` lists the narrow public schema and command surface.
- `vela_v09_sidon_kernel_fixture.py` and its JSON fixture exercise append,
  restrict, and observe on a small Sidon instance.
- `vela_no_hidden_state_check.py` and its pass/fail fixtures require every
  authoritative displayed value to have one replayable observation packet.

These executable vectors are protocol-surface checks. Formal claims live in
the separately scoped Lean sources and are not implied by a vector pass.

## Extending the set

When a reducer mutation kind changes:

1. update the fixture builders in
   `crates/vela-protocol/tests/cross_impl_reducer_fixtures.rs`;
2. run that focused Rust test to regenerate the affected fixtures;
3. copy the generated fixtures into `conformance/fixtures/`;
4. regenerate `fixtures.manifest.json`; and
5. run `python3 conformance/verify.py`.

The fixtures are licensed under the repository's Apache-2.0 OR MIT terms.
