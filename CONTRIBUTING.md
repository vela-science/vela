# Contributing to Vela

Vela is an open-source frontier-state protocol focused on proof-ready v0
(frontier-first, bounded claims, conservative outputs). v0 proves state, not
extraction.

## Before you start

- Keep edits scoped to the proof-ready wedge:
  - frontier bootstrap, check, proof, and serve
  - search and inspect over portable frontier state
  - proof packet and reviewability surfaces
- Do not position v0 as a full science operating system, lab runtime, autonomous agent loop, Hub, desktop app, or federation platform.
- Use `frontier` terminology in docs, examples, and APIs.
- When in doubt, treat outputs as candidate views unless reviewed and versioned.

## Contribution flow

1. Fork and open a branch.
2. Keep commits focused and descriptive.
3. Prefer small, reversible changes.
4. Include docs updates when behavior or capability language changes.
5. Open a PR with:
   - motivation and scope
   - proof-impact checklist (below)
   - validation notes

## Contributor checklist

- [ ] Does the change use frontier terminology (for example `frontier_stats`, `search_findings`, `frontier.json`)?
- [ ] Are experimental features labeled as candidate/heuristic when not fully reviewed?
- [ ] Is `compile` framed as bootstrap/onboarding rather than the trust anchor?
- [ ] Are claims limited to proof-ready scope (no broad federation/lab/runtime/agent-loop overclaiming)?
- [ ] Are evidence and caveat boundaries documented?
- [ ] Are changes covered by docs and tests relevant to the touched area?

## Release checklist

Before tagging a release candidate, complete:

- [ ] Run repository gates:
  - `cargo fmt --all -- --check`
  - `cargo check --workspace`
  - `cargo test --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo build --release -p vela-protocol`
  - `./target/release/vela --help`
  - `./target/release/vela stats frontiers/bbb-alzheimer.json`
  - `./target/release/vela check frontiers/bbb-alzheimer.json`
  - `./target/release/vela proof frontiers/bbb-alzheimer.json --out /tmp/vela-proof-packet`
  - `tests/test-http-server.sh`
  - `tests/test-mcp-server.sh`
  - `./demo/run-bbb-proof.sh`
- [ ] Verify v0 proof narrative stays aligned across:
  - `README.md`
  - `docs/CORE_DOCTRINE.md`
  - `docs/PROOF.md`
  - `docs/PROTOCOL.md`
  - `docs/MCP_SETUP.md`
- [ ] Confirm contribution checklist in this file and `AGENTS.md` are still accurate.
- [ ] Document any changes to public capability language.
- [ ] Sanity-check that the BBB/Alzheimer proof packet remains intact and conservative.
- [ ] Update release notes and version metadata before publishing.

## First useful contributions

- Add a small conformance fixture that exercises one schema rule.
- Improve one BBB proof finding with clearer provenance or confidence components.
- Add one benchmark task with a frozen gold answer and conservative scoring.
- Improve MCP docs with a reproducible frontier investigation transcript.
- Add a validator case for malformed proof packets.
