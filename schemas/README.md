# Vela JSON Schemas

One document describes the signed transport. `dsse-envelope.schema.json` is the
DSSE envelope every signed Vela
object is stored in — Submissions, Verification Records, producer Withdrawals
and repository-authority records alike. In accordance with DSSE, the envelope
and its signature entries permit unknown fields; each closed payload beneath
them is a document of its own:

- `vela.submission.v2`;
- `vela.verification-record.v2`; and
- `vela.proposal-withdrawal.v2`.

The envelope schema cannot pin `payloadType` to one value the way the payload
schemas each pin their `schema` tag, because one envelope serves four payload
types. It constrains the value to the `application/vnd.vela.*+json` namespace;
requiring the exact type belongs to — and has always been enforced by — the
reader for each object, which refuses a foreign type before it verifies a
signature.

And the three objects that carry the science:

- `vela.claim-record.v1`;
- `vela.proposal.v1`; and
- `vela.repository-origin.v1`.

Those three were unpublished until they were derived, which meant the schema
for a Claim Record could not be generated at all — the contract a reader most
needs to check a Claim without this implementation was the one contract the
repository did not state.

The repository and authority boundary is also published from its live types:

- `vela.repository-profile.v1`;
- `vela.authorization-request.v1`; and
- `vela.authorization-evaluation.v1`.

The Profile schema describes the decoded TOML structure whose canonical JSON
defines `profile_root`. Authorization request and evaluation are exact payloads
retained inside repository-authority records; an Allow remains authorization
evidence and is not a scientific Decision.

## Two are read surfaces

`status.schema.json` describes `vela.status.v4`, the document
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

`error.schema.json` describes `vela.error.v1`, the failure object every CLI
command emits under `--json`. Its stable `kind` selects the exit-code class and
its optional `code` names a machine-actionable refusal. Message text is prose.
The enriched preflight branch may additionally prove `changed: false` and name
the retained request identifier; it carries no authority or Standing effect.

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

`status.schema.json` states one rule its consumer does not have to restate:
every field is `required`, including the ones whose value is `null` on a
Frontier that cannot fill them. A bootstrapping repository has a Git pointer
with no commit behind it, not an absent Git pointer, and a schema that let the
key vanish would let a dropped field pass as an empty one.

Further rules live only in the readers, because JSON Schema cannot reach them:

- a Verification Record's `completed_at` may not precede its `started_at`;
- a Verification Record may not declare independence from its own `verifier`;
- text fields reject interior control characters, where the published pattern
  can only reject leading and trailing whitespace; and
- each object's encoded size ceiling (8 MiB, 4 MiB, and 2 MiB respectively);
- all three Repository Profile license values parse as SPDX license
  expressions, and its include/exclude sets do not overlap;
- Profile bounds are byte bounds and text is NFC, while JSON Schema length is
  measured in Unicode characters;
- an authorization evaluation's roots recompute from the retained request and
  model; and
- the request, operation, and retained identifiers in an enriched error
  envelope agree exactly.

## Every pattern is portable

No pattern in these files uses lookahead, backreferences, or any other
construct outside the portable regular subset, so an implementation may compile
them with a finite-automaton engine that has no backtracking at all.
`verify_patterns_are_portable` in `conformance/verify_wire_schemas.py` reads
every pattern in every published document and holds them to that.

The Artifact-path rule in `submission.schema.json` used to be the exception.
It rejected paths that escape the tree with two negative lookaheads, which
ECMA-262 and Python provide and Rust's `regex` does not, so the one published
pattern guarding path traversal was the one a Rust consumer could not compile.
The rule is now spelled as the structure it describes: components joined by
`/`, none of which is `..`.

The two spellings agree on every string with no line terminator in it, and
there the current one is exactly the rule the readers apply:
`safe_path_pattern_agrees_with_the_reader` in
`crates/vela-protocol/src/objects/submission.rs` settles that against
`require_safe_relative_path` over all 21845 strings of up to seven dots,
slashes, spaces and other characters, rather than over a sample of them. Where
a line terminator is present the two part, and it was the lookahead that was
wrong: `.` stops at a line terminator, so in `a\n/..` the `..` was never read
before the negative lookahead ran out, in ECMA-262 and in Python alike. The
current pattern rejects that path, as the readers always have.

No published object carries its own identifier. `vsb_`, `vvr_`, `vpr_`, `vpw_`
and `vro_` are the first sixteen hexadecimal digits of the object's full root,
derived by the reader; where one appears in a reference it sits beside the root
it came from and must re-derive from it. That is why the reference patterns
here are `^vsb_[0-9a-f]{16}$` rather than `^vsb_.+$`: a handle has exactly one
right value, and a truncated identifier that cannot be checked is the kind that
resolves to the wrong object.

Run the independent checks with:

```bash
uv run --project conformance --locked python conformance/verify_wire_schemas.py
```
