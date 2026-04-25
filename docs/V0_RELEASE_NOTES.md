# Vela v0.2.0 release notes

Vela v0.2.0 is the first proof-first release of the frontier state kernel.

The release is intentionally narrow: Vela represents bounded scientific
frontier state, accepts reviewed state transitions, replays correction history,
serves the state to tools, and exports proof packets. It does not claim to be a
trusted literature extractor, autonomous scientist, lab runtime, federation
network, Hub, desktop app, or full science operating system.

The release principle is:

> Vela v0 proves state, not extraction.

## What to evaluate

Evaluate whether Vela can make scientific claims durable, inspectable,
correctable, and replayable.

The core loop is:

```text
frontier.json
  -> proposal-backed correction
  -> accepted canonical event
  -> reducer/replayable frontier state
  -> stale proof detection
  -> refreshed proof packet
```

`compile` is useful for onboarding a corpus into candidate state. It is not the
trust anchor. Reviewed and accepted state transitions are the boundary for
frontier state.

## First ten minutes

Build the CLI:

```bash
cargo build --release -p vela-protocol
```

Inspect the canonical BBB/Alzheimer protocol sample:

```bash
./target/release/vela stats frontiers/bbb-alzheimer.json
./target/release/vela check frontiers/bbb-alzheimer.json --strict --json
./target/release/vela search "LRP1 RAGE amyloid" --source frontiers/bbb-alzheimer.json
./target/release/vela proof frontiers/bbb-alzheimer.json --out /tmp/vela-proof-packet
./target/release/vela packet validate /tmp/vela-proof-packet
```

Run the concise state-proof demo:

```bash
./demo/v0-state-proof-demo.sh
```

Run the full integration proof workflow:

```bash
./demo/run-bbb-proof.sh
```

The BBB sample is a protocol demonstration for state, review, replay, serving,
and proof mechanics. It is not an authoritative Alzheimer's BBB science map.

## Release artifacts

The release surface is:

- `frontiers/bbb-alzheimer.json`: canonical sample frontier
- `schema/`: finding-bundle schema
- `crates/vela-protocol/`: Rust CLI and MCP/HTTP server
- `docs/`: current v0 doctrine, protocol, proof, CLI JSON, and theory appendices
- `examples/paper-folder/`: small local corpus onboarding fixture
- proof packet assets and JSON projections generated from the canonical sample

The macOS and Linux release artifacts are CLI binaries only. They are not a
desktop app.

## What is not in v0

Do not evaluate v0 as:

- a complete BBB synthesis
- a general literature intelligence product
- a trusted automated extraction engine
- a scientist-facing workflow application
- a lab protocol/result runtime
- a federation or institution coordination layer
- an autonomous agent loop

Those are future layers above the state kernel.

## Known limits

- Confidence is frontier support for the represented finding, not truth
  probability.
- Candidate gaps, bridges, tensions, observer rerankings, and PubMed checks are
  review surfaces, not conclusions.
- `compile` can use deterministic fallback extraction when no model backend is
  configured; those outputs are review leads.
- `.vela/` directory storage remains a compatibility and Git-friendly layout;
  `frontier.json` is the first-class artifact for v0 evaluation.
