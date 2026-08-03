# Vela JSON Schemas

These JSON Schema 2020-12 documents describe Vela's current portable
producer, verifier, and authority-envelope structure:

- `vela.submission.v1`;
- `vela.verification-record.v1`; and
- `vela.proposal-withdrawal.v1`.

`authority-envelope-v1.schema.json` describes the current DSSE authority
envelope. In accordance with DSSE, the envelope and signature entries permit
unknown fields; the decoded Vela authority payload remains closed.

They are checked against deterministic fixtures in `conformance/`. They do not
replace the Rust readers and do not verify canonical bytes, object-derived
identifiers, Ed25519 signatures, referenced objects, actor relationships,
repository invariants, human Decision authority, or Standing. The schemas use
`format: date-time` as an assertion in Vela's conformance check.

The current objects still use their v1 signed-preimage contracts. A future
common DSSE transport is a separate v2 protocol cut under ADR 0035; these files
do not imply that migration has occurred.

Run the independent checks with:

```bash
uv run --project conformance --locked python conformance/verify_wire_schemas.py
```
