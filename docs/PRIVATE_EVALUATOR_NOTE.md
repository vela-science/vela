# Private evaluator note

Vela v0.2.0 is an internal pre-public release candidate. Please evaluate the
*state primitive* — the typed, replayable, reviewable kernel described below —
not the broader vision behind it. Where "state" appears in this doc it means
frontier state as a computable object (a finding bundle plus its canonical
event log), not "current status."

## What Vela v0 is

Vela v0 is a Git-native protocol for replayable scientific frontier state. It
stores bounded scientific findings as typed state: assertion, evidence,
conditions, entities, confidence, provenance, links, proposals, canonical
events, and proof metadata.

The v0 claim is:

> Vela can represent frontier state, accept reviewed state transitions, replay
> corrections, and invalidate stale proofs.

The release principle is:

> Vela v0 proves state, not extraction.

## What to run first

Build the CLI:

```bash
cargo build --release -p vela-protocol
```

Run the short proof demo:

```bash
./demo/v0-state-proof-demo.sh
```

That demo works on a temporary copy of the BBB sample and shows:

```text
frontier state
-> proof packet
-> reviewed correction
-> canonical event history
-> stale proof detection
-> refreshed proof packet
```

Then run the first-frontier onboarding fixture:

```bash
examples/paper-folder/run.sh
```

That flow shows how a small local corpus becomes candidate frontier state,
quality reports, normalized projections, reviewed transitions, proof artifacts,
and tool-serving output.

For the full local gate:

```bash
./scripts/release-check.sh
```

## What not to evaluate it as

Please do not judge this release as:

- a trusted automated literature extractor
- an authoritative Alzheimer's BBB map
- an autonomous scientist
- a lab runtime
- a federation or institution network
- a Hub, desktop app, or science operating system
- a general-purpose Semantic Scholar/Elicit/SciSpace replacement

`compile` is onboarding. Reviewed state transitions are the trust boundary.

## What feedback is useful

Useful feedback:

- Does the state primitive make sense?
- Does the correction -> replay -> stale proof loop feel credible?
- Is the first 10 minutes confusing anywhere?
- Are proposal-backed writes and canonical events legible?
- Would this be useful as shared working memory for humans and agents?
- What would an agent/tool builder need next from the read surface?

Less useful feedback:

- whether the tiny BBB sample is scientifically complete
- whether fallback extraction captures every finding
- whether the project should immediately add runtime, federation, or UI

Those are outside v0.
