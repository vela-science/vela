# Stage A Anthropic neutral calibration v4 terminal evidence

The independently approved v4 Anthropic neutral permit was released and consumed exactly once. The provider returned a schema-valid terminal response. There was exactly one endpoint-attempt receipt and no retry or substitution. The provider, bridge, runner, controller terminal, and custody records all derive and report `provider_calls: 1` from that receipt.

The lossless request transport preserved the exact 4,278 committed pre-frame request bytes through the canonical base64 outer frame, single bridge decode, and endpoint write. All representations bind to `sha256:cf67944d1872244c9d89ed3f7ad9cc27c3a37a4deba665f47a939985e2c62e8c`. The exact 2,384-byte mounted provider schema occurs once in the request actually written to the endpoint. No JSON parse or reserialization occurred in the transport boundary.

This terminal result remains pending independent review and is not a positive qualification. The consumed v4 permit cannot be retried or reused. The permanent v2 consumed non-call and v3 consumed failed-exact-request records remain immutable in the denominator. The OpenAI neutral permit and all twelve participant permits remain held. There was no participant call, scoring, Stage B selection, Protocol/Core action, authority effect, Decision, or Standing.

The artifact retains the complete non-secret request frame, actual network body, endpoint-attempt receipt, provider event and response bytes, parsed response, usage, terminal, teardown, and credential non-retention custody. The credential was supplied only by inherited descriptor, the descriptor was closed, its buffer was scrubbed, and secret bytes are not retained.

`execution-build.json` binds the exact source and binary identities plus the deterministic Go build parameters. In particular, the Darwin host bridge is linked to the frozen `anthropic-messages-v1` adapter; omitting that link binding is a different binary and fails its recorded digest.

The package is closed to exactly the regular, single-link files listed in `artifact-root.json`, plus the manifest itself. Symlinks, hardlinks, special files, undeclared files or directories, caches, manifest omissions, manifest additions, duplicate JSON keys, unknown fields, boolean-as-integer substitutions, and fully resealed custody drift fail verification.

Run:

```bash
uv run --project conformance --locked python -B paper/artifacts/lean-correspondence-stage-a-anthropic-neutral-calibration-v4/verify.py
uv run --project conformance --locked python -B -m unittest discover -s paper/artifacts/lean-correspondence-stage-a-anthropic-neutral-calibration-v4 -p 'test_*.py' -v
```
