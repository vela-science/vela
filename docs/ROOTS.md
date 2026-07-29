# Roots and identifiers

Vela uses typed commitments. The text `sha256:` identifies an algorithm and
encoding; the containing schema and field identify what was hashed.

## Rules

1. Security comparisons use the full digest, root kind, and canonicalization
   profile.
2. Readable prefixes such as `vfr_`, `vcl_`, `vsb_`, `vrr_`, `vvr_`, `vpr_`,
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

Current Vela JSON uses `canonical::to_canonical_bytes`:

- object keys sort lexicographically at every depth;
- array order remains semantic;
- no insignificant whitespace is emitted;
- strings use JSON escaping with UTF-8 preserved;
- numbers use the pinned `serde_json` round-trip form; and
- non-finite values are rejected.

This is the Vela canonical JSON profile, not a claim of universal RFC 8785
equivalence. `conformance/canonical-hashing.json` pins the bytes.

Artifact digests hash exact bytes. Git commit and tree IDs use Git's object
format. DSSE and OpenSSH signatures use their separately versioned signing
inputs. These profiles are not interchangeable.

## Current catalogue

| Commitment | Meaning | Not a substitute for |
| --- | --- | --- |
| Frontier ID `vfr_…` | Stable bounded repository identity | Git commit, repository root, or Standing |
| Git commit | Exact commit object and ancestry | Tree, valid Vela state, or acceptance |
| Git tree | Exact tracked paths and bytes | Commit ancestry or authority |
| Epoch ID/root | Current repository epoch and predecessor boundary | Repository root or authority head |
| Repository root | Canonical current object-set commitment | Git commit, authority record, or Claim Standing |
| Authority-record root | Full DSSE transaction record commitment | Trust-anchor choice or scientific truth |
| Authority trust-anchor root | Local closed record selecting sequence one | Secret key, later freshness, or Standing |
| Authority Event root | Exact semantic Event content | Authority-record signature or Event-log root |
| Authority Event-log root | Ordered current authority Event set | Repository root or accepted Claim set |
| Claim ID `vcl_…` | Full content-derived current Claim identity | Standing or Proposal |
| Claim Record root | Complete canonical Claim Record | Claim ID alone or acceptance |
| Submission ID/root `vsb_…` | Authenticated producer input | Verification, Registration, or Decision |
| Registration ID/root `vrr_…` | Exact Vela intake record | Inclusion, correctness, or acceptance |
| Verification ID/root `vvr_…` | Signed scoped verifier observation | Broader truth or authority |
| Proposal ID/root `vpr_…` | Candidate transition | Decision or Event |
| Artifact digest | Exact retained evidence bytes | Scientific meaning or availability elsewhere |
| Target Index root | Derived current producer catalogue | Work authority or Standing |
| Target-task binding root | Exact Target, packet, source, and starting roots | Verification or Decision |
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

### Registration Record

The readable Registration handle derives from the canonical record with its
self-ID cleared. The full root covers the complete Vela-issued record.

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

## Repository epoch commitments

`vela.repository-epoch.v1` binds the exact predecessor:

- remote, tag, commit, and tree;
- predecessor canonical roots;
- Git-object manifest root;
- archive-bundle SHA-256;
- imported Claim set root;
- retained current-object set root;
- archived-object index root; and
- equivalence-report root.

The current repository root commits to the current epoch and canonical object
sets. Current bytes do not substitute for predecessor signatures. The epoch
proves the mapping and retained Standing transition.

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
- A Target Index rederives but names an old repository root: it is stale and
  yields no Offer.

## Conformance

Implementations must reproduce exact canonical bytes and roots, not merely
agree with themselves:

```bash
cargo test -p vela-protocol --test canonical_hashing_conformance
cargo test -p vela-protocol --test current_object_interop
python3 conformance/verify.py
```
