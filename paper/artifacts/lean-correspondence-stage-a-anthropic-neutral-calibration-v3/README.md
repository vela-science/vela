# Stage A Anthropic neutral calibration v3 terminal evidence

The independently approved replacement Anthropic neutral permit was released and consumed exactly once. The provider returned a schema-valid terminal response, with one endpoint-attempt receipt and exact agreement on `provider_calls: 1` across controller, bridge, runner, terminal, and custody. There was no retry or substitution.

This is nevertheless a failed exact-request calibration and a `non_result`, not a positive qualification. The runner's pre-frame request was 4,278 bytes, while the body actually extracted from the provider-request frame and transmitted by the bridge was 3,363 bytes. They decode to the same JSON value but are not byte-identical. The frozen formatted provider-schema bytes occur once in the pre-frame request and zero times in the transmitted body; its compact semantic equivalent occurs zero and one times respectively. The cause is Go `encoding/json` compaction of a `json.RawMessage` while marshaling the outer provider-request frame.

Every original v3 raw byte and the consumed permit remain unchanged. `raw/actual-transmitted-body.raw.json` is a deterministic slice of the retained runner-to-bridge frame, independently extracted by `extract_actual_request.py`. `terminal-outcome.json` is the authoritative qualification classification and binds both request representations, the successful provider response, the terminal permit disposition, and the stopped state. Exact raw provider response/event bytes, parsed response/text, usage, packet and schema bindings, process teardown, and credential non-retention receipts are closed by the verifier. Secret bytes are not retained.

The earlier v2 consumed non-call evidence remains immutable and remains in the denominator. The OpenAI neutral permit and all twelve participant permits remain held and were not released. The consumed v3 permit is terminal and cannot be retried or reused. There was no participant call, scoring, Stage B selection, Protocol/Core action, authority effect, Decision, or Standing.

Prospective lossless byte-payload transport and any fresh replacement permit are explicitly not implemented here. This amendment does not authorize another request.

The artifact is closed to exactly the regular single-link files listed in `artifact-root.json`, plus the manifest itself. Symlinks, hardlinks, special files, undeclared files/directories, caches, manifest omissions, and manifest additions fail verification.

Run:

```bash
uv run --project conformance --locked python -B paper/artifacts/lean-correspondence-stage-a-anthropic-neutral-calibration-v3/verify.py
uv run --project conformance --locked python -B -m unittest discover -s paper/artifacts/lean-correspondence-stage-a-anthropic-neutral-calibration-v3 -p 'test_*.py' -v
```
