# Vela Python conformance client

This directory is a repository-local independent reader, reducer, and verifier.
It exists to catch cross-implementation drift. It is not a second Vela writer,
SDK product, package-distribution promise, or authority source.

## Contents

- `vela_loader.py` reads a standalone frontier and replays its accepted events.
- `vela_reducer.py` implements the reducer subset exercised by the shared
  cross-implementation fixtures.
- `vela_verify.py` and `vela_verify_log.py` perform independent structural and
  log checks.
- `tests/test_loader_frontiers_v2.py` loads the checked-in Erdős reference
  frontier and checks dependency, event, finding, and frontier-ID projection.

The Rust implementation under `crates/vela-protocol` is the current reference.
Wire behavior is defined by `docs/PROTOCOL.md` and conformance fixtures, not by
Python module names.

## Run

```bash
python3 clients/python/tests/test_loader_frontiers_v2.py
python3 -m pytest clients/python/tests/
```

## Scope

The client checks a deliberately bounded projection rather than reimplementing
every derived Vela view. It does not hold keys, evaluate human policy intent,
land receipts, publish Git history, register Hub sources, or write accepted
events. Unknown event kinds fail closed instead of disappearing silently.

When the Rust and Python results disagree, treat it as a conformance failure and
compare both implementations with the normative protocol and fixture generator.
Do not paper over version skew or mutate the event log to make the projections
match.
