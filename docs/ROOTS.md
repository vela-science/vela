# Roots and identifiers

Vela uses typed commitments. The text `sha256:` identifies an algorithm and
encoding; the containing schema and field identify what was hashed.

## Rules

1. Security comparisons use the full digest, root kind, and canonicalization
   profile.
2. Readable prefixes such as `vfr_`, `vcl_`, `vsb_`, `vvr_`, `vpr_`,
   and `vev_` are routing handles, not interchangeable security roots.
3. Roots from different domains remain different typed commitments even when
   their 64 hexadecimal characters coincide.
4. Missing, malformed, shortened, ambiguous, or differently typed roots fail
   closed.
5. Git objects, Vela roots, signatures, verifier outcomes, and Standing answer
   different questions.
6. Derived projections name their exact source roots and never substitute
   their own digest for a canonical root.

## Canonical bytes

Current Vela protocol JSON uses RFC 8785 JSON Canonicalization Scheme (JCS)
through `canonical::to_canonical_bytes`:

- object keys sort by UTF-16 code units at every depth;
- array order remains semantic;
- no insignificant whitespace is emitted;
- strings use the RFC 8785 JSON escaping rules;
- numbers use ECMAScript number serialization; and
- duplicate properties, non-finite values, and protocol integers outside the
  exact I-JSON/IEEE-754 safe-integer range are rejected before hashing.

`conformance/canonical-hashing.json` and the official RFC 8785 vectors pin the
bytes. Raw scientific and execution evidence remains hashed as exact retained
bytes; it is not parsed and silently recanonicalized as protocol JSON.

Artifact digests hash exact bytes. Git commit and tree IDs use Git's object
format. DSSE and OpenSSH signatures use their separately versioned signing
inputs. These profiles are not interchangeable.

## Current catalogue

| Commitment | Meaning | Not a substitute for |
| --- | --- | --- |
| Frontier ID `vfr_…` | Stable bounded repository identity | Git commit, repository root, or Standing |
| Git commit | Exact commit object and ancestry | Tree, valid Vela state, or acceptance |
| Git tree | Exact tracked paths and bytes | Commit ancestry or authority |
| Origin ID/root | Immutable repository origin and optional predecessor provenance | Repository root or authority head |
| Repository root | Canonical current object-set commitment | Git commit, authority record, or Claim Standing |
| Authority-record root | Full DSSE transaction record commitment | Trust-anchor choice or scientific truth |
| Authority trust-anchor root | Local closed record selecting sequence one | Secret key, later freshness, or Standing |
| Authority Event root | Exact semantic Event content | Authority-record signature or Event-log root |
| Authority Event-log root | Ordered current authority Event set | Repository root or accepted Claim set |
| Claim ID `vcl_…` | Full content-derived current Claim identity | Standing or Proposal |
| Claim Record root | Complete canonical Claim Record | Claim ID alone or acceptance |
| Submission ID/root `vsb_…` | Authenticated producer input retained by an exact Proposal reference | Verification or Decision |
| Verification ID/root `vvr_…` | Signed scoped verifier observation | Broader truth or authority |
| Proposal ID/root `vpr_…` | Candidate transition | Decision or Event |
| Artifact digest | Exact retained evidence bytes | Scientific meaning or availability elsewhere |
| Target Index root | Derived current producer catalogue | Work authority or Standing |
| Projection root | One derived reader artifact | Any source root |
| Release checksum | Exact binary/archive bytes | Source commit or build attestation |

## Current object identities

### Claim Record

The current Claim identity is content-derived from its version, assertion,
conditions, evidence, and provenance. Relations use full Claim identities.
The full Claim Record root also covers relation metadata and source provenance.

### Submission

The readable Submission handle derives from the canonical signed preimage with
the self-ID and authentication signature cleared. The full root covers the
complete authenticated object.

### Verification Record

The readable Verification handle derives from its canonical signed preimage
with self-ID and signature cleared. The full root covers the complete
authenticated record.

### Proposal

The Proposal identity commits to the logical requested transition and exact
subject objects. The full Proposal root covers the complete current canonical
record and status.

### Authority Event

The Event content root covers schema, kind, target, principal attribution,
time, reason, before/after roots, payload, and caveats. The covering authority
record signature remains a separate check.

## Repository origin commitments

`vela.repository-origin.v1` binds a native genesis or one exact compacted
pre-release predecessor. A compacted origin commits to the predecessor remote,
tag, commit, tree, repository and authority roots, archive digest, Git-object
manifest root, and equivalence-report root.

The current `vela.repository.v4` root commits to that origin and every active
canonical object set. Current bytes do not substitute for predecessor
signatures; the predecessor remains independently inspectable through its tag,
archive, and pinned historical release.

## Comparison contract

- Full Vela SHA-256 roots are lowercase `sha256:` plus exactly 64 hexadecimal
  characters.
- A handle resolves only when exactly one matching object rederives its full
  commitment.
- Event timestamps and `created_at` never establish membership.
- Git ancestry proves publication continuity; Vela replay proves repository
  validity. Claims of continuity usually need both.
- A derived view may change while its canonical source roots remain fixed.
- Canonical objects may not be rewritten as “materialization.”
- A valid authority chain still requires the independently installed
  sequence-one trust root.

## Adversarial examples

- Two objects share a readable prefix: resolution is ambiguous and fails.
- A valid Git commit contains malformed Vela objects: Git preserves the bytes;
  strict Vela replay rejects them.
- A Verification Record passes for a different property: its full subject and
  scope bindings do not match and acceptance remains unavailable.
- A site bundle is internally valid but names an old repository root: it is a
  valid historical projection, not the current state.
- A checkout contains a valid authority chain but no pinned sequence-one root:
  it has not selected the intended authority fork.
- A tracked Target Index names an old repository root or mismatched
  input/packet bytes: it is stale and yields no Offer.

## Conformance

Implementations must reproduce exact canonical bytes and roots, not merely
agree with themselves:

```bash
cargo test -p vela-protocol --test canonical_hashing_conformance
cargo test -p vela-protocol --test current_object_interop
uv run --project conformance --locked python conformance/verify.py
```
