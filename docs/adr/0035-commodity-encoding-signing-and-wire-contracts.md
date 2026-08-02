# ADR 0035: Commodity encoding, signing, and wire contracts

- Status: Proposed
- Proposed: 2026-08-02
- Protocol effect: pre-1.0 canonical-byte and signed-record reset
- Scientific effect: none; no Verification becomes acceptance and no Standing
  changes without an attributed Decision
- Interoperability effect: exact JCS, DSSE, JSON Schema 2020-12, and retained
  RO-Crate 1.3 export

## Context

Vela should own scientific meaning, replay, correction, Decision, and
Standing. It should not own a novel JSON canonicalizer or signature envelope.

The current implementation has four different standards postures:

1. `vela.canonical-json/v1` is deterministic but is not RFC 8785 JCS. It
   sorts Rust strings by Unicode scalar value and preserves `serde_json`
   number forms. JCS sorts raw UTF-16 code units and uses ECMAScript number
   serialization over I-JSON values.
2. Repository-authority records use the DSSE shape and PAE correctly for the
   current Vela profile. Submission, Verification, identity, revocation, and
   producer withdrawal instead sign bespoke zeroed-field preimages directly.
3. Rust types and semantic validators close the current objects, but external
   producers have no published JSON Schema 2020-12 contract.
4. Cedar is already restricted to repository authority. The retained
   RO-Crate 1.3 experiment is already a derived transfer view and does not
   create Standing.

The current DSSE implementation also has four envelope-level interoperability
gaps even though its signatures and PAE are sound: it accepts only standard
base64, rejects unknown envelope fields, requires `keyid`, and aborts on any
unknown or invalid extra signature. DSSE 1.0.2 requires standard and URL-safe
base64 acceptance, treats `keyid` as optional and unauthenticated, tells
consumers to ignore unknown envelope fields, and permits threshold verification
to skip signatures that do not verify.

The current Cedar boundary is correctly narrow, but historical verification
does not re-run Cedar. It verifies the signed Authority Record's retained
`Allow` result, diagnostics, engine/profile/version, and policy-bundle root.
The exact request and entity inputs are represented only by roots, so the
authorization result is an authenticated attestation rather than an
independently recomputed policy evaluation.

The canonicalization difference is observable. For object keys `U+E000` and
`U+1F600`, the current Rust/Python order places `U+E000` first, while JCS
places the non-BMP key first because its leading UTF-16 surrogate is smaller.
The current vectors also intentionally encode `1.0` as `1.0` and allow a
`serde_json` construction path to coerce NaN to `null`; JCS emits `1` and
requires non-finite input to fail.

Current portable Submission and Verification fixtures contain only values for
which both encodings agree. That fact is useful but does not prove that all
retained Claim extensions, Event payloads, authority payloads, or repository
objects agree.

## Decision

Adopt the following narrow standards spine before 1.0:

```text
storage and ancestry       Git
object bytes               RFC 8785 JCS
object identity            SHA-256 over exact JCS bytes
signed transport           DSSE 1.0.x + Ed25519
portable structure         JSON Schema 2020-12
repository authorization   restricted Cedar
scientific meaning         Vela Decision, Event, Standing, correction
research-object transfer   RO-Crate 1.3 adapter
```

### 1. Replace canonicalization with exact JCS

Use one maintained RFC 8785 implementation, pinned by exact dependency and
license, rather than repairing the current recursive sorter into another
Vela-specific canonicalizer.

Before switching a writer:

1. add the official RFC number, string, escaping, UTF-16 ordering, and invalid
   input vectors;
2. run both encoders over every retained canonical object in all four
   Frontiers, including decoded authority-record payloads;
3. record every byte and root difference in one deterministic audit; and
4. verify the same JCS bytes with independent Rust, JavaScript, and Python
   readers.

JCS inputs must satisfy I-JSON. Numeric protocol fields must either be proven
within the exact IEEE-754 safe-integer range or encoded as strings. Arbitrary
extension values that cannot satisfy this boundary fail before hashing.

If all retained objects agree, replace the implementation without changing
their roots. If any retained object differs, use one explicit current-state
repository cut. Do not ship dual canonicalizers in the current runtime.

### 2. Use one DSSE envelope for signed Vela objects

Introduce one shared DSSE envelope implementation and use a distinct,
lowercase application payload type for each signed payload. The envelope must:

- implement the exact DSSE PAE;
- accept standard and URL-safe base64 as required by DSSE;
- treat `keyid` only as an unauthenticated key-selection hint;
- verify the exact payload bytes once and pass those same bytes to the payload
  parser;
- accept unknown envelope fields as required by the DSSE envelope contract;
  and
- keep Vela payload schemas closed and fail on unknown payload fields.

The current authority envelope becomes a user of the common implementation.
Submission, Verification Record, and producer Withdrawal move to versioned
DSSE payloads. The signed payload carries the actor, class, public-key or
credential binding, exact scientific content, and all semantic limitations.
The outer signature proves possession; a nested second raw signature must not
repeat the same fact. If a standalone identity lifecycle remains necessary,
its signed objects use the same DSSE boundary.

DSSE changes transport authentication only. It does not grant repository
authority, select a scientific outcome, or change Standing.

Before introducing any v2 payload, make the existing authority verifier
DSSE-compatible without changing emitted bytes: accept both base64 alphabets,
tolerate unknown envelope fields, make `keyid` optional, and count only unique
trusted signatures that actually verify. This is a parser and conformance
repair, not a repository migration.

The unused `IdentityRevocation` type has no writer, storage route, reducer, or
verification consumer. Delete it unless the v2 credential model identifies a
real revocation consumer. Do not preserve a ceremonial lifecycle in source.

### 3. Publish JSON Schema 2020-12 for portable objects

Publish closed schemas with stable `$id` values for the DSSE envelope and each
portable payload. Start with the actual write boundary:

```text
submission
verification-record
proposal-withdrawal
```

Then publish reader schemas for Claim, Decision/authority payloads, and any
other object that an independent reader actually consumes.

Schemas must use the exact 2020-12 meta-schema, close nested objects, bound
arrays and strings, constrain identifiers and roots, and state whether any
`format` is annotation or assertion. Tracked schemas are generated or checked
from the Rust contract and must not become an independently edited source of
truth.

JSON Schema validates structure. Rust continues to enforce referenced-object
existence, exact roots, signatures, authority, correction targets, scientific
semantics, and repository invariants.

### 4. Keep Cedar contained

Cedar remains restricted to human Decisions and repository administration.
It is not an ordinary evidence gate, workflow permission system, scientific
acceptance language, hosted authorization service, or Agent Campaign policy.

Historical Cedar bundles remain exact authenticated-attestation inputs. A
future current-schema cut may remove Cedar only if a smaller standard
authorization boundary preserves the same historical and authority invariants.

For new authority records, retain the exact canonical Cedar request and entity
snapshot needed to recompute authorization, bind them by root, and re-evaluate
them during strict history verification with the recorded policy bundle and
engine profile. If that cost is rejected, documentation must say plainly that
Cedar authorization is signed and attested rather than replayed. Do not claim
independent policy replay from roots that cannot reconstruct the request.

### 5. Retain RO-Crate as the research-object adapter

Keep the existing RO-Crate 1.3 transfer experiment, metadata, reader, loss
report, and tests. RO-Crate packages evidence and context; it does not replace
Submission, Verification, Decision, Event, or Standing.

The current artifact is honestly scoped as a Decision-chain transfer package.
Its next earned improvement is a deterministic, allowlisted crate archive that
contains or immutably resolves the evidence a named receiver needs. A Vela
profile or importer is justified only after a real consumer needs a stable
shape. Removing the current RO-Crate work is not part of this decision.

## Migration

This is one pre-1.0 standards cut, not a compatibility program.

1. Freeze the four current Frontier heads and audit JCS parity.
2. Repair the existing authority DSSE parser against the official envelope and
   protocol vectors without changing emitted bytes.
3. Implement and independently test the common DSSE/JCS boundary.
4. Generate and validate the portable JSON Schemas.
5. Close, archive, or explicitly carry every pending Proposal before the
   repository cut; no agent makes the scientific choice.
6. Build one exact replacement current state for each controlled Frontier.
7. Preserve accepted Claims, Decision meaning, and Standing; bind any changed
   evidence roots through the administrative cut rather than pretending old
   signers re-signed new bytes.
8. Strictly replay clean clones, then delete the old writers and readers from
   the current runtime. Historical Git and pinned binaries preserve old bytes.
9. Update the RO-Crate transfer view to carry the current signed envelopes
   without treating them as receiver authority.

No old signature may be copied into a new DSSE envelope, and no agent may
manufacture a replacement producer, verifier, or human signature.

## Conformance

The migration is complete only when tests prove:

- official RFC 8785 vectors pass in each maintained implementation;
- duplicate properties, lone surrogates, unsafe numbers, NaN, and Infinity
  fail before hashing;
- all four pre-cut repositories have a complete byte/root parity report;
- DSSE PAE matches the official vectors;
- standard and URL-safe base64 verify;
- unknown envelope fields and non-threshold invalid signatures do not block a
  valid trusted threshold;
- payload-type substitution, payload drift, duplicate signer use, unknown
  keys, insufficient threshold, and post-verification payload substitution
  fail closed;
- `keyid` alone never authorizes a signer;
- every public payload passes both its JSON Schema and Rust semantic validator;
- unknown nested payload fields fail, including identity fields;
- each new Cedar-backed authority record can reproduce its authorization result
  from the retained request, entities, policy bundle, and engine profile, or is
  explicitly classified as an attested non-replayable historical result;
- Submission and Verification still change no accepted Event or Standing;
- only an attributed human Decision changes Standing;
- the four post-cut Frontiers strictly replay from clean clones; and
- the retained RO-Crate reader reports no local authority effect.

## Consequences

The change removes bespoke security machinery and gives workbenches ordinary,
documented wire contracts. It also creates a real pre-1.0 repository cut,
because signed bytes cannot be rewritten invisibly.

The migration should be performed once and followed by deletion. Permanent
dual readers, legacy aliases, and compatibility writers are rejected while
Vela has no external users.

## Rejected alternatives

### Keep `vela.canonical-json/v1`

Rejected. Its documented differences now affect a cryptographic boundary for
which RFC 8785 already exists.

### Add DSSE around the current signed object unchanged

Rejected. Double-signing preserves the bespoke zeroed-field protocol instead
of replacing it.

### Generate schemas from a second modeling language

Rejected. LinkML, SHACL, and a Vela ontology are not needed for a small JSON
wire boundary.

### Replace Vela semantics with RO-Crate or provenance standards

Rejected. Packaging and provenance are evidence inputs, not scientific
authority or Standing.

## References

- RFC 8785, JSON Canonicalization Scheme:
  <https://www.rfc-editor.org/rfc/rfc8785.html>
- DSSE protocol and envelope 1.0.2:
  <https://github.com/secure-systems-lab/dsse>
- JSON Schema 2020-12:
  <https://json-schema.org/draft/2020-12>
- Cedar authorization:
  <https://docs.cedarpolicy.com/auth/authorization.html>
- RO-Crate 1.3:
  <https://www.researchobject.org/ro-crate/specification/1.3/index.html>
