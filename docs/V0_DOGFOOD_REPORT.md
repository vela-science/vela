# v0 dogfood report

This report records the first post-release internal dogfood pass against the
small paper-folder frontier. It is a tooling and protocol check, not a
scientific evaluation.

Run:

```bash
cargo build --release -p vela-protocol
examples/paper-folder/run.sh
```

Output directory:

```text
/tmp/vela-first-frontier
```

## Result

The first-frontier workflow now passes end to end.

- local corpus compile: 4 sources accepted, 0 skipped, 0 errors
- generated findings: 11
- generated links: 11
- strict check: pass, 11 valid findings, 0 invalid findings
- normalization: 3 safe confidence updates, 0 unsafe changes
- reviewed transitions: 2 applied proposals
- canonical events: 1 `finding.reviewed`, 1 `finding.caveated`
- event replay: ok
- proof packet validation: ok
- MCP/HTTP tool check: 9 checks passed, 0 failed, 10 registered tools

## What felt good

The useful v0 loop is visible in a small corpus:

```text
compile candidate state
-> inspect quality reports
-> normalize deterministic projections
-> apply reviewed corrections
-> inspect history
-> export and validate proof
-> serve the same state to tools
```

The fixture is small enough that a reviewer can inspect the generated findings
and understand which outputs are candidates, which changes are accepted state,
and where the proof boundary begins.

## Rough edge found and fixed

The initial dogfood run exposed a replay failure:

```text
Proof trace replay_status must be ok or no_events
```

Root cause: the example workflow applied review/caveat events, then ran
`normalize`. Normalization recomputes deterministic finding fields and can
therefore change finding hashes after canonical events already exist.

Fix:

- the first-frontier flow now normalizes before proposal-backed writes
- `normalize --out` and `normalize --write` now refuse eventful frontiers
- docs now state that normalization is a pre-write repair step in v0

This keeps the v0 event log honest: once canonical events exist, durable state
changes should be represented as proposal-backed transitions.

## Remaining product notes

- The paper-folder workflow is onboarding, not proof of extraction quality.
- The fallback extractor is conservative and incomplete without a model backend.
- The quality reports are useful, but still feel like engineer-facing artifacts.
- `normalize` is the right safety valve before review, but longer term it may
  need its own reviewed event type if post-review normalization becomes common.
- The strongest demo remains the BBB correction -> stale proof -> refreshed
  proof path in `demo/v0-state-proof-demo.sh`.
