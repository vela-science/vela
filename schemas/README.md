# Vela JSON Schemas

These JSON Schema 2020-12 documents describe Vela's current portable
producer, verifier, and authority-envelope structure:

- `vela.submission.v1`;
- `vela.verification-record.v1`; and
- `vela.proposal-withdrawal.v1`.

And the three objects that carry the science:

- `vela.claim-record.v1`;
- `vela.proposal.v1`; and
- `vela.repository-origin.v1`.

Those three were unpublished until they were derived, which meant the schema
for a Claim Record could not be generated at all — the contract a reader most
needs to check a Claim without this implementation was the one contract the
repository did not state.

`authority-envelope-v1.schema.json` describes the current DSSE authority
envelope. In accordance with DSSE, the envelope and signature entries permit
unknown fields; the decoded Vela authority payload remains closed.

## One of these is a read surface

`status-v4.schema.json` describes `vela.status.v4`, the document
`vela status --json` answers with. It signs nothing and roots nothing. It is
published because a second implementation parses it: the Observatory in
`vela-web` builds its whole projection from this document, and until this file
existed the only thing holding the two shapes together was running the refresh
and watching it fail. Three shape changes reached that consumer that way in one
week — `counts.withdrawn_review` and `git.role` arriving, and
`actions.work.mode` becoming a union.

Everything a read surface reports is derived, so nothing here is evidence of
anything. `integrity.replay` says what replay found on the machine that ran it;
a consumer that needs to establish that for itself runs `vela replay`.

## These files are generated

Do not edit them. They are produced from the Rust types in
`crates/vela-protocol/src/objects/` and `crates/vela-protocol/src/read_surface/`
— for the signed objects, the only implementation that builds canonical bytes
and signs, and so normative. Regenerate after changing a type:

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

`status-v4.schema.json` states one rule its consumer does not have to restate:
every field is `required`, including the ones whose value is `null` on a
Frontier that cannot fill them. A bootstrapping repository has a Git pointer
with no commit behind it, not an absent Git pointer, and a schema that let the
key vanish would let a dropped field pass as an empty one.

Four further rules live only in the readers, because JSON Schema cannot reach
them:

- a Verification Record's `completed_at` may not precede its `started_at`;
- a Verification Record may not declare independence from its own `verifier`;
- text fields reject interior control characters, where the published pattern
  can only reject leading and trailing whitespace; and
- each object's encoded size ceiling (8 MiB, 4 MiB, and 2 MiB respectively).

## Every pattern is portable

No pattern in these files uses lookahead, backreferences, or any other
construct outside the portable regular subset, so an implementation may compile
them with a finite-automaton engine that has no backtracking at all.
`verify_patterns_are_portable` in `conformance/verify_wire_schemas.py` reads
every pattern in every published document and holds them to that.

The Artifact-path rule in `submission-v1.schema.json` used to be the exception.
It rejected paths that escape the tree with two negative lookaheads, which
ECMA-262 and Python provide and Rust's `regex` does not, so the one published
pattern guarding path traversal was the one a Rust consumer could not compile.
The rule is now spelled as the structure it describes: components joined by
`/`, none of which is `..`.

The two spellings agree on every string with no line terminator in it, and
there the current one is exactly the rule the readers apply:
`safe_path_pattern_agrees_with_the_reader` in
`crates/vela-protocol/src/objects/submission_v1.rs` settles that against
`require_safe_relative_path` over all 21845 strings of up to seven dots,
slashes, spaces and other characters, rather than over a sample of them. Where
a line terminator is present the two part, and it was the lookahead that was
wrong: `.` stops at a line terminator, so in `a\n/..` the `..` was never read
before the negative lookahead ran out, in ECMA-262 and in Python alike. The
current pattern rejects that path, as the readers always have.

The current objects still use their v1 signed-preimage contracts. A future
common DSSE transport is a separate v2 protocol cut under ADR 0035; these files
do not imply that migration has occurred.

Run the independent checks with:

```bash
uv run --project conformance --locked python conformance/verify_wire_schemas.py
```
