# Stage A Anthropic neutral calibration v3 terminal evidence

The independently approved replacement Anthropic neutral permit was released and consumed exactly once. The one provider request completed successfully, with one endpoint-attempt receipt and exact agreement on `provider_calls: 1` across controller, bridge, runner, terminal, and custody. There was no retry or substitution.

The retained response conforms to the frozen Stage A participant schema. Exact request bytes, raw provider response/event bytes, usage, packet and schema bindings, process teardown, and credential non-retention receipts are retained. Secret bytes are not retained.

The earlier v2 consumed non-call evidence remains immutable and remains in the denominator. The OpenAI neutral permit and all twelve participant permits remain held and were not released. There was no participant call, scoring, Stage B selection, Protocol/Core action, authority effect, Decision, or Standing.

The artifact is closed to exactly the regular single-link files listed in `artifact-root.json`, plus the manifest itself. Symlinks, hardlinks, special files, undeclared files/directories, caches, manifest omissions, and manifest additions fail verification.

Run:

```bash
uv run --project conformance --locked python -B paper/artifacts/lean-correspondence-stage-a-anthropic-neutral-calibration-v3/verify.py
uv run --project conformance --locked python -B -m unittest discover -s paper/artifacts/lean-correspondence-stage-a-anthropic-neutral-calibration-v3 -p 'test_*.py' -v
```
