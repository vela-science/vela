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

## Receipt v1 compatibility contract

The emitter keeps producer claims neutral. A reported verifier run receives
`producer_reported` status, while acceptance and artifact assessment remain
`not_assessed` with `acceptance_scope: hypothesis_only`. The tool does not name
an acceptor or infer a signature, identity assurance, or policy decision.

New receipts bind the full receipt body, excluding only `attestation`, into the
in-toto predicate at `vela:receipt_body.sha256`. The validator checks that root,
the duplicated predicate projection, and the DSSE payload. It accepts an older
receipt without the body field as `legacy_unbound`, but a malformed or stale
binding fails validation.

The JSON reader rejects duplicate object names at every depth, including names
written with Unicode escapes and names inside the decoded DSSE payload. This
prevents different parsers from interpreting the same bytes as different
receipts.

Receipt v1 remains frozen at 15,458 schema bytes with SHA-256
`369eed995d8a430a7d7b37e1431f04b01301b1c2789d541b3f4a32221088bf93`.
Run `scripts/check-receipt-schema-sync.sh` from the repository root to compare
every bundled copy byte for byte. Run `python3 scripts/cross_impl_conformance.py`
to exercise the independent Python and JavaScript readers.
