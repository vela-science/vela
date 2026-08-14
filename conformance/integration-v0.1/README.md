# Integration contract v0.1 conformance

This corpus tests the Phase 0 contract and candidate Phase 2 structural waist.
It is not listed by `conformance/protocol-1.json` and changes no Protocol 1 object.

Each rooted document is canonical JSON with its own root field replaced by the
empty string, prefixed by its UTF-8 schema tag plus NUL, and hashed with SHA-256.
`generate_fixtures.py` owns `fixtures.json`; regeneration must be byte-identical.
The generic Profile is synthetic. Source-specific Profiles stay with their
native repositories.

The packet's `check_output` is an unrooted synthetic test value used only to
exercise refusal behavior for authority claims, acceptance claims, and
unavailable evidence. INT-00 does not define a shared result schema or root
domain.

```bash
uv run --project conformance --locked python conformance/integration-v0.1/generate_fixtures.py --check
uv run --project conformance --locked python conformance/integration-v0.1/verify.py
```
