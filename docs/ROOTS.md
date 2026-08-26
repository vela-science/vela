# Roots and identifiers

Vela uses typed commitments. The text `sha256:` identifies an algorithm and
encoding; the containing schema and field identify what was hashed.

## Rules

1. Security comparisons use the full digest, root kind, and canonicalization
   profile.
2. The Repository identifier is a lowercase RFC 9562 UUIDv4. Readable prefixes
   such as `vcl_`, `vsb_`, `vvr_`, `vpr_`, and `vev_` are routing handles, not
   interchangeable security roots. A prefix is never reused across epochs:
   `vfr_` names an epoch-1 repository and stays bound to it.
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
| Repository UUID | Stable bounded repository identity | Git commit, repository root, or Standing |
| Git commit | Exact commit object and ancestry | Tree, valid Vela state, or acceptance |
| Git tree | Exact tracked paths and bytes | Commit ancestry or authority |
| Origin ID/root `vro_…` | Immutable repository genesis | Repository root or authority head |
| Repository root | Canonical current object-set commitment | Git commit, authority record, or Claim Standing |
| Authority-record root | Canonical decoded `AuthorityRecordV1` payload commitment; its retained DSSE envelope is verified separately | Trust-anchor choice, envelope byte root, or scientific truth |
| Authority trust-anchor root | Local closed record selecting sequence one | Secret key, later freshness, or Standing |
| Authority Event root | Exact semantic Event content | Authority-record signature or Event-log root |
| Authority Event-log root | Ordered current authority Event set | Repository root or accepted Claim set |
| Claim ID `vcl_…` | Full content-derived current Claim identity | Standing or Proposal |
| Claim Record root | Complete canonical Claim Record | Claim ID alone or acceptance |
| Submission ID/root `vsb_…` | Signed producer input retained by an exact Proposal reference | Verification or Decision |
| Verification ID/root `vvr_…` | Signed scoped verifier observation | Broader truth or authority |
| Proposal ID/root `vpr_…` | Candidate transition | Decision or Event |
| Artifact digest | Exact retained evidence bytes | Scientific meaning or availability elsewhere |
| Projection root | One derived reader artifact | Any source root |
| Release checksum | Exact binary/archive bytes | Source commit or build attestation |

Source-local next-obligation and work-packet roots belong to their owning
schema, not this Vela root catalogue. A producer may retain selected packet,
manifest, method, or output bytes as Submission Artifacts and one opaque native
run reference as provenance; none grants authority or Standing.

## Current object identities

### Claim Record

The current Claim identity is content-derived from its version, assertion,
conditions, evidence, and provenance. Relations use full Claim identities.
The full Claim Record root also covers relation metadata and source provenance.

### Submission, Verification Record, Proposal Withdrawal

These three are DSSE envelopes and share one rule. The full root is `sha256:`
over the canonical envelope bytes exactly as retained — payload type, payload
and signatures together — and the readable handle is the prefix plus the first
sixteen hexadecimal characters of that root. Nothing is cleared and nothing is
reconstructed: the bytes that were signed and the bytes that were hashed are
the bytes on disk.

No object stores its own handle. A handle appearing in one object as a
reference to another is checked by re-deriving it from a full root present in
the same object, so `subject.submission_id` is only readable beside
`subject.submission_root`. A handle with nothing to re-derive from is a value
no reader can check, and the protocol does not carry one.

### Proposal

The Proposal identity commits to the logical requested transition and exact
subject objects. The full Proposal root covers the complete current canonical
record, and `vpr_` derives from it. Proposal status is not stored in that
record; it is derived from Proposal withdrawals and governed Decision Events.

### Authority Event

The Event content root covers schema, kind, target, principal attribution,
time, reason, before/after roots, payload, and caveats. The covering authority
record signature remains a separate check.

## Repository origin commitments

`vela.repository-origin.v1` binds a native genesis. It commits to the
repository identity, the Profile root, the generation, the initial object-set
root and the reason the lineage was opened, and `vro_` derives from the origin
root. It has no predecessor fields: a pre-release compaction once carried
eleven of them, and continuity across a future lineage change belongs in a
separately signed attestation over exact commits, trees and roots rather than
as permanent fields on every repository's origin.

The current `vela.repository.v4` root commits to that origin and every active
canonical object set. An archived predecessor remains independently
inspectable through its tag, archive, and pinned historical release.

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

## Conformance

Implementations must reproduce exact canonical bytes and roots, not merely
agree with themselves:

```bash
cargo test -p vela-protocol --test canonical_hashing_conformance
cargo test -p vela-protocol --test object_interop
uv run --project conformance --locked python conformance/verify.py
```
