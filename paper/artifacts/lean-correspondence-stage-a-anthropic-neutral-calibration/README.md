# Stage A Anthropic neutral calibration terminal evidence

The one authorized Anthropic neutral-calibration permit was atomically consumed once. The attempt then failed closed before request construction because the staged run input embedded a semantically identical but byte-different copy of the frozen provider schema. No provider endpoint was contacted, no response or usage existed, and no retry occurred.

The raw controller receipt incorrectly hardcoded `provider_calls: 1`; it is retained byte-for-byte as non-authoritative raw custody. `terminal-outcome.json` is the authoritative causal classification and binds the exact mismatch, zero provider calls, terminal permit consumption, teardown, and unchanged participant/OpenAI stopped state.

The smallest prospective correction is a reviewed run-input materializer that inserts the exact mounted provider-schema bytes as a raw JSON value, followed by an offline runner input-validation and pre-request gate before permit consumption. The controller must also remove its hardcoded provider-call counter: a call may increment only from a successful endpoint-write/request-attempt receipt, with exact consistency required across controller, bridge, runner, and custody records. This evidence does not implement that successor and does not authorize a retry.

All twelve participant permits and the OpenAI neutral permit remain held. There was no scoring, key disclosure, Stage B selection, Protocol/Core action, authority effect, Decision, or Standing.

The artifact is closed to exactly the regular single-link files listed in `artifact-root.json`, plus the manifest itself, and to the four directories derived from those paths. Symlinks, hardlinks, special files, undeclared directories, Python caches, manifest omissions, and manifest additions fail verification. No transient path is allowed or silently ignored.

Run verification and tests with bytecode disabled before any test module import:

```bash
uv run --project conformance --locked python -B \
  paper/artifacts/lean-correspondence-stage-a-anthropic-neutral-calibration/verify.py
uv run --project conformance --locked python -B -m unittest discover \
  -s paper/artifacts/lean-correspondence-stage-a-anthropic-neutral-calibration \
  -p 'test_*.py' -v
```

`seal.py` shares the verifier's exact file and directory inventory and sets `sys.dont_write_bytecode` before importing it. It refuses to reseal if any undeclared transient or other path exists.
