# Vela JSON Schemas

These JSON Schema 2020-12 documents describe Vela's current portable
producer, verifier, and authority-envelope structure:

- `vela.submission.v1`;
- `vela.verification-record.v1`; and
- `vela.proposal-withdrawal.v1`.

`authority-envelope-v1.schema.json` describes the current DSSE authority
envelope. In accordance with DSSE, the envelope and signature entries permit
unknown fields; the decoded Vela authority payload remains closed.

## These files are generated

Do not edit them. They are produced from the Rust object types in
`crates/vela-protocol/src/objects/`, which are the only implementation that
builds canonical bytes and signs, and so are normative. Regenerate after
changing a type:

```bash
VELA_BLESS_WIRE_SCHEMAS=1 cargo test -p vela-protocol --test wire_schemas
```

`cargo test` fails when a checked-in file and the current types disagree, so a
field added to a struct and not regenerated here stops CI rather than leaving
a schema that quietly describes the previous release.

## What they do not check

They are also checked against deterministic fixtures in `conformance/`. They do
not replace the Rust readers and do not verify canonical bytes, object-derived
identifiers, Ed25519 signatures, referenced objects, actor relationships,
repository invariants, human Decision authority, or Standing. The schemas use
`format: date-time` as an assertion in Vela's conformance check.

Four further rules live only in the readers, because JSON Schema cannot reach
them:

- a Verification Record's `completed_at` may not precede its `started_at`;
- a Verification Record may not declare independence from its own `verifier`;
- text fields reject interior control characters, where the published pattern
  can only reject leading and trailing whitespace; and
- each object's encoded size ceiling (8 MiB, 4 MiB, and 2 MiB respectively).

`submission-v1.schema.json` also uses regular-expression lookahead to reject
Artifact paths that escape the tree. Lookahead is available in ECMA-262 and
Python validators but is outside the portable subset, so a validator built on a
lookahead-free engine cannot compile that one pattern.

The current objects still use their v1 signed-preimage contracts. A future
common DSSE transport is a separate v2 protocol cut under ADR 0035; these files
do not imply that migration has occurred.

Run the independent checks with:

```bash
uv run --project conformance --locked python conformance/verify_wire_schemas.py
```
