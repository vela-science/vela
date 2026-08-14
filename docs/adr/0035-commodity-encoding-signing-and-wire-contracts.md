# ADR 0035: Commodity encoding, signing, and wire contracts

- Status: Accepted and implemented; the current-epoch cut completed 2026-08-09
- Proposed: 2026-08-02
- Accepted: 2026-08-09
- Protocol effect: pre-1.0 canonical-byte and signed-record reset
- Scientific effect: none; no Verification becomes acceptance and no Standing
  changes without an attributed Decision
- Interoperability effect: exact JCS, DSSE, JSON Schema 2020-12, and retained
  RO-Crate 1.3 export

## Context

Vela should own scientific meaning, replay, correction, Decision, and
Standing. It should not own a novel JSON canonicalizer or signature envelope.

At proposal time, the implementation had four different standards postures.
The implementation note below records which parts have since shipped and which
remain open:

1. `vela.canonical-json/v1` was deterministic but was not RFC 8785 JCS. It
   sorts Rust strings by Unicode scalar value and preserves `serde_json`
   number forms. JCS sorts raw UTF-16 code units and uses ECMAScript number
   serialization over I-JSON values.
2. Repository-authority records use the DSSE shape and PAE correctly for the
   current Vela profile. Submission, Verification, identity, revocation, and
   producer withdrawal instead sign bespoke zeroed-field preimages directly.
3. Rust types and semantic validators close the current objects, but external
   producers have no published JSON Schema 2020-12 contract.
4. Cedar is restricted to repository authority, but its general policy
   language, schemas, entity snapshots, bundle rotation, and engine-version
   lifecycle duplicate a much smaller rule already enforced by Vela: one
   authenticated human with a fixed role may request one closed authority
   action on one exact resource. The retained RO-Crate 1.3 experiment is
   already a derived transfer view and does not create Standing.

At proposal time, the DSSE implementation also had four envelope-level
interoperability gaps even though its signatures and PAE were sound: it
accepted only standard
base64, rejects unknown envelope fields, requires `keyid`, and aborts on any
unknown or invalid extra signature. DSSE 1.0.2 requires standard and URL-safe
base64 acceptance, treats `keyid` as optional and unauthenticated, tells
consumers to ignore unknown envelope fields, and permits threshold verification
to skip signatures that do not verify.

The Cedar boundary remains narrow, but historical verification does not re-run
Cedar. It verifies the signed Authority Record's retained
`Allow` result, diagnostics, engine/profile/version, and policy-bundle root.
The exact request and entity inputs are represented only by roots, so the
authorization result is an authenticated attestation rather than an
independently recomputed policy evaluation.

The retired canonicalization difference was observable. For object keys
`U+E000` and `U+1F600`, the old Rust/Python order placed `U+E000` first, while JCS
places the non-BMP key first because its leading UTF-16 surrogate is smaller.
The current vectors also intentionally encode `1.0` as `1.0` and allow a
`serde_json` construction path to coerce NaN to `null`; JCS emits `1` and
requires non-finite input to fail.

The portable Submission and Verification fixtures at proposal time contained
only values for which both encodings agreed. That fact was useful but did not prove that all
retained Claim extensions, Event payloads, authority payloads, or repository
objects agree.

### Shadow-audit result: 2026-08-02

The frozen parsed-value shadow audit used pinned
`serde_json_canonicalizer 0.3.2` against clean checkouts at:

- Erdős `8428650c47c0dcb5429e6621a6f023a878fc42c8`;
- Formal Conjectures `100d0028bb5b4714ddace4812a77a7ad617ac97c`;
- Quantum Codes `718de33dcdb27e97e92458530e938f2262c86fbe`;
  and
- Sidon Sets `d2b7480d404e9e0edbf750798fc33896efe41270`.

The audit recursively rejected duplicate JSON properties before
canonicalization, compared 3,161 parsed tracked JSON values, and separately
decoded and compared all seven retained authority-record payloads. Of the
tracked values, 3,158 were byte-identical under current Vela canonicalization
and JCS. Every decoded authority payload was byte-identical. The only three
differences were raw scientific or execution evidence, not canonical Vela
protocol objects:

- `attack/canopus-trace-306.eval.json`;
- `attack/erdos1093-deficiency-search-k129.v2.json`; and
- the archived `erdos573-incidence-construction.v1.json` source artifact.

All 17 integers outside the interoperable IEEE-754 safe-integer range occur in
the Erdős 1093 raw evidence artifact. Those values must remain exact raw bytes
or use strings in any future portable Vela object; JCS must never round them
silently. `conformance/jcs-shadow-audit.json` binds the exact repository
commits and trees, counts, seven authority payloads, raw exception byte hashes
and Git blobs, first-difference offsets, unsafe-integer counts, and canonical
result root. The result supports a root-preserving canonicalizer switch only
for the compared parsed values and authority payloads at those four heads. It
does not prove that arbitrary future extension values satisfy I-JSON or
authorize later wire-contract changes.

### Implementation note: 2026-08-02

The production protocol now uses pinned `serde_json_canonicalizer 0.3.2`,
passes the official RFC 8785 vectors, rejects duplicate properties recursively,
and refuses unsafe protocol integers before hashing. An independent Python
reader uses pinned `rfc8785 0.1.4` through the committed uv lock. The four
canonical Frontier roots remained unchanged through the switch. Authority
records now use DSSE 1.0.2-compatible envelope parsing and threshold behavior.

At proposal time the dependency-free Vela Authorization Profile evaluator
existed only in shadow mode. A frozen migration corpus bound the epoch-1 heads
and all seven retained authority transactions, reproduced each Cedar request
root, and proved Allow parity plus seven negative boundary cases. No denied
Cedar evaluation was ever published, and the shadow evaluator was not called
by the writer. That temporary corpus was removed after the current authority
model and independent signed-chain vector replaced the migration boundary.

This ADR remains Proposed because the common DSSE boundary for Submission and
Verification, portable JSON Schema 2020-12 contracts, retained model/request
history, and the explicit current-epoch cut have not shipped. Cedar must not be
deleted until strict history recomputes the closed evaluation and all four
replacement Frontiers replay from clean clones.

### Implementation note: 2026-08-09

Accepted. The remaining four items shipped together, as one wire break, because
each of them separately would have forced another `vela-science/math`
re-genesis.

§2 is implemented as written. `crates/vela-protocol/src/kernel/dsse.rs` is the
only DSSE implementation in the tree; `EnvelopeV1` carries PAE, both base64
alphabets, tolerant parsing and the threshold loop, and authority records,
Submissions, Verification Records and Proposal Withdrawals are all typed users
of it. The zeroed-field preimage convention is gone: no `signed_preimage`, no
`derive_id` that hashes one, and no nested second signature anywhere. Each
parser verifies the exact payload bytes once and hands those same bytes to the
strict payload parser. `IdentityBinding` lost its ceremony and became
`SignerIdentityV1` — a declaration of who is signing and under which key,
proved by the envelope signature rather than by a signature of its own.

§3 shipped as `crates/vela-protocol/src/wire_schema.rs`, generating the eight
published schemas under `schemas/` with a drift gate, and two independent
emitters that reproduce the fixtures byte for byte.

§4's history gap is closed. `AuthorityRecordV1` retains the exact authorization
request, and `verify_record_authorization` recomputes the decision under the
rooted model instead of trusting a retained `Allow`. `PolicyBundleV1` is
`AuthorizationModelV1`, which names no engine and no engine version, so a
future evaluator change is a code change rather than a signature problem.
Before any of that was deleted, the epoch-1 parity corpus that ADR 0042 noted
"is read by nothing" was wired into a temporary Rust test. It recomputed all
seven retained Cedar Allows under the closed profile and checked seven negative
boundary cases for their exact reasons. Once the current signed-chain vector
covered the live authority contract, the migration-only corpus and test were
removed rather than retained as a compatibility path.

This ADR also supersedes ADR 0042, and it is worth saying why, because 0042
asked the right question and this cut answered it by accident. 0042 holds that
Cedar cannot be deleted while `vela-science/math` retains a signed
`PolicyBundleV1` naming the pinned evaluator: the reader refuses any bundle
that disagrees, so the live repository must be rotated to new policy material
*before* the reader stops accepting the old, and no rotation writer exists. All
of that is accurate. What it does not consider is that a wire break removes the
repository the bundle is retained on. §2 moves the signature preimage for
Submission and Verification Record, `math` must re-genesis, and genesis mints a
fresh authority chain and a fresh model from the binary performing it. There is
then no retained bundle left to contradict a Cedar-free reader, and no rotation
to sequence. 0042's own §"Sequencing against ADR 0035" gets within one step of
this — it observes that doing both in one cut costs "one operator ceremony
instead of two" — without noticing that the second ceremony has nothing left to
do.

This is not 0042's rejected alternative of archiving `math` to avoid writing a
verb. There the objection was that discarding signed history to save a feature
leaves the next rotation in the same position; here the re-genesis is forced by
the wire contract and would happen whether or not Cedar were involved, and it
is paid once for the whole pre-1.0 cut rather than once per retirement. The
rotation writer remains unwritten and is not owed.

The current-epoch cut is the one thing that did not ship, and cannot from here.
`vela-science/math` must re-genesis under the new contract, which needs the
authority key in a local OpenSSH agent. Until an operator performs it, the
binary refuses the current `math` head with a schema error — the same
sequencing as release 0.970.0.

**Closed 2026-08-09.** The operator performed the re-genesis under Vela
0.972.1, so the paragraph above describes a state that no longer exists.
`vela-science/math` is generation 1 at repository UUID
`8115c538-7688-40b7-ab75-3c4765bf3c19` with origin root
`sha256:229ce0a08217da5e8bad2059c35070989652ca546ab45b8e699922ba182e8a69`, and
it strictly replays: the observation is recorded in `ecosystem-status.json`,
which `conformance/verify.py` holds to the checkout on every run. Its signed
0.971.0 predecessor is retained as continuity evidence and carries no Standing
forward. Nothing in this ADR is now waiting on an operator, and the sequencing
note is kept as written because it is why the cut was ordered the way it was.

## Decision

Adopt the following narrow standards spine before 1.0:

```text
storage and ancestry       Git
object bytes               RFC 8785 JCS
object identity            SHA-256 over exact JCS bytes
signed transport           DSSE 1.0.x + Ed25519
portable structure         JSON Schema 2020-12
authorization boundary     AuthZEN 1.0 information model
repository authorization   closed Vela Authorization Profile v1
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

The unused `IdentityRevocation` type had no writer, storage route, reducer, or
verification consumer and was deleted in the first standards slice. A future
credential lifecycle must identify a real revocation consumer before adding a
replacement. Do not preserve a ceremonial lifecycle in source.

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

### 4. Replace Cedar with a closed AuthZEN-shaped authority profile

OpenID Authorization API 1.0 standardizes the authorization boundary as
subject, action, resource, context, and decision while deliberately leaving
application policy and action meaning to the implementation. Vela adopts that
information shape, not a network policy-decision service.

The canonical evaluator is a closed **Vela Authorization Profile v1** with only
the roles and actions exercised today:

```text
administrator  authority_initialize authority_rotate authority_close
               authority_model_update
reviewer       review_accept review_reject
```

The authority model binds one exact Frontier and sorted human members to those
fixed roles. Unknown model versions, actions, roles, members, resources, or
context fields fail closed. The exact request is retained with the authority
record and strict history verification recomputes the decision under the exact
rooted model. This fixes the current Cedar history gap, where replay verifies a
signed `Allow` attestation but cannot reconstruct the complete evaluation.

Authorization answers only whether the authenticated subject may request the
action on the exact resource. Verification eligibility, current-root checks,
semantic approval, correction rules, and the resulting Standing transition
remain separate deterministic transaction and reducer preconditions. An
authorization `Allow` is never a scientific Decision.

Reuse the existing intent-bound `SemanticApprovalV1` and repository-authority
DSSE signature. Do not add signed grant/revocation objects, a configurable
quorum system, arbitrary conditions, a policy editor, or another signature
scheme. W3C Verifiable Credentials 2.0 and Bitstring Status List 1.0 may become
derived role-credential adapters only after real cross-institutional use.
GNAP and OAuth Rich Authorization Requests remain deferred until Vela has an
actual hosted API delegation requirement. CEL, Biscuit, OpenFGA, SpiceDB, OPA,
XACML, and FROST are rejected for the current repository-authority boundary.

Before the cut, run the closed evaluator offline against every retained
Cedar-backed fixture and transaction and require identical Allow/Deny results,
plus exact resource binding that the generated Cedar policy currently delegates
to later semantic checks. Do not ship both evaluators in the current runtime.

### 5. Retain RO-Crate as the research-object adapter

Keep the existing RO-Crate 1.3 transfer experiment, metadata, reader, loss
report, and tests. RO-Crate packages evidence and context; it does not replace
Submission, Verification, Decision, Event, or Standing.

The current artifact is honestly scoped as a Decision-chain transfer package.
It carries the exact source transition and uses a closed fixity manifest. A
standard archive, repository, OCI artifact, or deposit may transport that file
set; Vela does not maintain another archive format. A Vela RO-Crate profile or
importer is justified only after a real consumer needs a stable shape.

## Migration

This is one pre-1.0 standards cut, not a compatibility program.

1. Freeze the four current Frontier heads and audit JCS parity.
2. Repair the existing authority DSSE parser against the official envelope and
   protocol vectors without changing emitted bytes.
3. Implement and independently test the common DSSE/JCS boundary.
4. Generate and validate the portable JSON Schemas.
5. Shadow the closed Authorization Profile against every retained Cedar-backed
   transaction and fixture; require decision parity and exact resource binding.
6. Close, archive, or explicitly carry every pending Proposal before the
   repository cut; no agent makes the scientific choice.
7. Build one exact replacement current state for each controlled Frontier. The
   new sequence-one authority chain installs the rooted authority model; the
   predecessor tag and pinned binary retain historical Cedar verification.
8. Preserve accepted Claims, Decision meaning, and Standing; bind any changed
   evidence roots through the administrative cut rather than pretending old
   signers re-signed new bytes.
9. Strictly replay clean clones, then delete Cedar, its crate/runtime, policy
   material, actions, writers, and readers from the current runtime. Historical
   Git and pinned binaries preserve old bytes.
10. Verify and, only if necessary, update the retained RO-Crate transfer view
   after the standards cut without treating its envelopes as receiver
   authority.

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
- before the cut, the closed evaluator matches every retained Cedar fixture
  and transaction;
- wrong principal/class/frontier/resource/root, unknown action/role/model,
  stale model, changed read set, and missing or mismatched semantic approval
  fail closed;
- strict history verification recomputes the AuthZEN-shaped decision from the
  exact request and rooted model rather than trusting a recorded result;
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

### Replace Cedar with another general policy engine

Rejected. OPA, XACML, OpenFGA, SpiceDB, and similar systems preserve or enlarge
the machinery Vela is deleting. AuthZEN supplies the interoperable request and
decision boundary; Vela owns only the small domain profile and transition
semantics.

### Make portable credentials or delegated API tokens canonical authority now

Rejected. VC 2.0, Bitstring Status Lists, GNAP, and OAuth RAR solve real
interoperability problems, but Vela currently has one local human authority and
no hosted authorization server. Add them only as earned adapters for a real
cross-institutional credential or delegated API consumer.

## References

- RFC 8785, JSON Canonicalization Scheme:
  <https://www.rfc-editor.org/rfc/rfc8785.html>
- DSSE protocol and envelope 1.0.2:
  <https://github.com/secure-systems-lab/dsse>
- JSON Schema 2020-12:
  <https://json-schema.org/draft/2020-12>
- OpenID Authorization API 1.0:
  <https://openid.net/specs/authorization-api-1_0.html>
- W3C Verifiable Credentials Data Model 2.0:
  <https://www.w3.org/TR/vc-data-model-2.0/>
- RFC 9635, GNAP, and RFC 9396, OAuth Rich Authorization Requests:
  <https://www.rfc-editor.org/rfc/rfc9635.html>
  <https://www.rfc-editor.org/rfc/rfc9396.html>
- RO-Crate 1.3:
  <https://www.researchobject.org/ro-crate/specification/1.3/index.html>
