# Scientific State Profile v1

A conformance target for implementations outside this repository.

This document selects; it does not describe. Every contract below is already
defined in `docs/PROTOCOL.md`, `docs/INTEROPERABILITY.md` or a schema in
`crates/vela-protocol`, and this profile adds no object, no field and no
vocabulary. What it adds is a **bar**: the seven contracts an implementation
must satisfy to exchange scientific state with a Vela repository, each paired
with the check that decides whether it does.

If a statement here and a statement in `PROTOCOL.md` disagree, `PROTOCOL.md` is
correct and this file is a bug.

## Why a profile at all

`INTEROPERABILITY.md` answers "what can be exchanged". An implementer building
against Vela asks a narrower question: *what exactly must I get right, and how
will I know?* Answering it in prose invites partial conformance that reads as
complete. Answering it as a table of contract → schema → check does not.

## The seven contracts

| # | Contract | Wire schema | Defined in | Held by |
|---|---|---|---|---|
| 1 | Producer | `vela.submission.v2` in a DSSE envelope | PROTOCOL.md §3.3 | `conformance/current-objects/submission-draft.json`, `verify_current_objects.py` |
| 2 | Artifact | Content address only — `sha256:<64 hex>` under `records/artifacts/sha256/` | PROTOCOL.md §3.6, ROOTS.md | `verify_canonical_hashing.py` |
| 3 | Verification | `vela.verification-record.v2` in a DSSE envelope | PROTOCOL.md §3.4 | `conformance/current-objects/verification-draft.json`, `verify_current_objects.py` |
| 4 | Authority | `vela.authority-record.v1`, `vela.authorization-model.v1` | THREAT_MODEL.md, SIGNING.md | `crates/vela-cli/tests/review_acceptance.rs` — **no language-independent vector** |
| 5 | Correction | `vela.correction-impact-input.v1` → `vela.correction-impact-projection.v1` | ADR 0004, CLI.md § Corrections | `conformance/fixtures/correction/`, `verify_correction_impact.py` |
| 6 | Projection | Root-bound derived rows | INTEROPERABILITY.md § Public read contracts | `conformance/readers/python/repository_root.py` |
| 7 | Canonical bytes | RFC 8785 JCS, SHA-256 | ROOTS.md | `verify_canonical_hashing.py`, `conformance/emitters/` |

Contract 7 is not one of the six object contracts; it is the one every other
contract rests on, and it is listed because an implementation that gets it
wrong fails all six without any of them looking wrong.

## What conformance means, per contract

### 1. Producer

Emit a DSSE envelope of payload type
`application/vnd.vela.submission.v2+json` whose canonical root matches the one
this repository computes over the same bytes. The root is over the envelope, so
it covers the payload, the payload type and the signatures together; there is
no separate object root and no zeroed preimage to reconstruct. `conformance/current-objects/submission-draft.json`
is the de facto producer contract — it is the exact draft the CLI signs — and
naming it as such is preferable to inventing a second description of the same
shape.

A conforming producer never emits `frontier` or `target`. The schema is closed
and rejects them. The receiving repository makes the association.

**Not required:** importing Vela's Event, authority or repository
implementation. `conformance/emitters/javascript.mjs` and
`conformance/emitters/python.py` are clean-room emitters that build the DSSE
envelope from first principles — PAE, base64, Ed25519 — and produce
byte-identical Submissions without any of it, and they agree with each other:
one independent implementation shows the specification is followable, two show
it is followable the same way.

### 2. Artifact

An Artifact has no envelope. It is bytes, addressed by `sha256:` of those
bytes, referenced from a Claim Record's `evidence` by root. There is nothing to
implement beyond hashing correctly, which is contract 7.

Stated as its own contract because implementations reliably invent one — an
artifact manifest, a media-type field, a registry — and each invention is a
place where two implementations can disagree about what a Claim rests on.

### 3. Verification

Emit a DSSE envelope of payload type
`application/vnd.vela.verification-record.v2+json`. Three payload fields decide
conformance and are the three most often filled in badly:

- `scope.property` — the single named question this record answers. Not the
  subject, not the method: the question.
- `scope.does_not_establish` — at least one limit, and the protocol requires
  it (`verification_record.rs`). A record that establishes everything
  establishes nothing checkable.
- `independence.declared_independent_of` — the actors whose work this checked.
  A verifier that shares a dependency with the producer discloses it rather
  than claiming independence it does not have.

A Verification Record changes no Standing. An implementation that treats a
passing record as acceptance is not conforming; it is a different protocol.

### 4. Authority

Only a signed Decision by the repository authority moves Standing. An
implementation that consumes Vela state must therefore be able to:

- verify a `vela.authority-record.v1` chain from the sequence-one record; and
- obtain that sequence-one root through a channel other than the repository it
  is verifying (`vela authority trust pin`).

An implementation that produces Vela state does not need authority at all.
Producing and verifying are unprivileged; deciding is not. That asymmetry is
the point of the boundary, and a profile that let a producer sign its own
acceptance would be describing a different system.

**This contract has no language-independent conformance vector, and that is a
gap rather than an omission.** `crates/vela-cli/tests/review_acceptance.rs`
executes a real authorized acceptance over a contiguous record chain, but it is
Rust and it exercises this implementation. Contracts 1, 3 and 7 each have
fixtures a foreign implementation can be held to; this one does not, so an
implementation claiming to verify authority is currently taking its own word
for it.

`conformance/fixtures/epoch1/authorization-profile-parity.json` is now read —
`crates/vela-authority/tests/authorization_profile_parity.rs` recomputes every
retained epoch-1 decision under the closed profile and checks seven negative
boundary cases. That closes the gap this paragraph used to name in its last
sentence, but not the one it names in its first: the corpus is a fixture a
foreign implementation *could* be held to, and the test that reads it is still
Rust. ADR 0041 is where the language-independent vector belongs.

### 5. Correction

`corrects` and `supersedes` are the two relation kinds acceptance acts on
(`CORRECTION_RELATION_KINDS`); accepting a Claim carrying one retires exactly
one accepted predecessor. Every other relation kind is retained description and
moves no Standing.

The impact of a correction is derived, never asserted: given the claim set, the
relation set and the transition, `vela.correction-impact-projection.v1` is a
function of those bytes. `vela correction impact` computes it over a real
repository and `conformance/fixtures/correction/` holds the derivation to fixed
vectors.

**A limit an implementer must know about, and it is a decision rather than an
oversight.** The projection traverses `depends` and `supports` claim-to-claim
edges. `vela.submission.v2` gives a producer no way to declare either, so the
current write path authors correction relations and nothing else; every
`depends` edge in the retained corpus came from the epoch-1 ingest.

That absence is ADR 0004's, which is titled *Falsify the need for a scientific
dependency primitive* and resolved to express dependency through the existing
narrow waist — Git identity, canonical roots, the proposal-to-authority
boundary — rather than by adding a primitive to a signed object. An
implementation conforming to this profile can therefore record a correction and
cannot record a cascade, and `vela correction impact` reports the empty cascade
truthfully rather than inferring one.

Driving the first correction end to end supplied evidence in that lane: the
projection wants an edge the waist does not carry. Whether that falsifies ADR
0004's position or confirms it — a repository that needs no cascade is a
repository the waist serves — is not settled here, and v1 does not pre-empt it.
ADR 0043's `claim-dependency-profile.v0` is a noncanonical, `requires`-only
experiment over frozen fixtures. It is not one of this profile's seven
contracts, a Claim relation, or evidence of a real accepted-state cascade.

### 6. Projection

Any derived read surface must bind every row to the root it was derived from,
and must not present a derived value as a retained one. `INTEROPERABILITY.md`
Rule B governs. `conformance/readers/python/repository_root.py` recomputes a
repository's root from a clean clone with no network and no Vela code; it reads
one file and says so on its own output. Reproducing the full retained object
set is `vela replay`, and there is no second implementation of that.

A projection that cannot say which root a row came from is not a projection of
this protocol.

### 7. Canonical bytes

RFC 8785 JSON Canonicalization Scheme, then SHA-256, rendered `sha256:<64
lowercase hex>`. `conformance/verify_canonical_hashing.py` holds it to vectors,
and `conformance/jcs-shadow-audit.json` records where a naive JSON encoder
diverges.

Get this wrong and every root differs, which at least fails loudly. Get it
*nearly* right — key ordering by UTF-16 code unit rather than code point, or
number formatting — and most objects agree while a few do not, which does not.

## What this profile deliberately excludes

A workbench runtime, a model router, a task scheduler, a package registry, a
universal ontology, a hosted write authority. None is required to exchange
scientific state, and each would make this profile a description of a product
rather than of a boundary.

Federation between two repository authorities is also out of scope for v1. It
needs one authority to have been driven through real decisions first, and that
has only just started happening.

## Versioning

This profile is `v1` and is additive-only within v1: a later revision may add a
contract or tighten a check, and may not change the meaning of one already
here. A change that would break an implementation conforming to v1 is `v2`,
with its own file.

Object schemas version independently, under the rules in
`INTEROPERABILITY.md § Versioning`. This profile names schemas; it does not own
them.
