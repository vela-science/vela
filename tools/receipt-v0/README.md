# Receipt v0 emitter

Dependency-free Python tools for external producers that need to emit and
validate `vela.receipt.v1` JSON without importing the Vela workspace.

## Emit

```bash
python3 -m vela_receipt_v0 emit \
  --claim "a(17) >= 104 under the submitted Sidon witness" \
  --artifact witnesses/a17.witness.json:witness \
  --caveat "Pending Vela landing and human acceptance." \
  --replayability exact \
  --out receipt.json
```

Artifact paths are written exactly as provided. `vela land` resolves them
relative to the frontier being landed into.

## Validate

```bash
python3 -m vela_receipt_v0 validate receipt.json
```

Validation is intentionally stricter than older `vela land` parsing: new
external emitters must include status, provenance, environment, caveats, and an
explicit replayability class.
