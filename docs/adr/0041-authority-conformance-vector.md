# ADR 0041: A language-independent conformance vector for the authority contract

- Status: Proposed
- Protocol effect: none. A vector describes existing bytes and adds no object,
  field or vocabulary
- Product effect: none on the binary; `conformance/verify.py` gains a check
- Authority effect: none, and the design question is entirely about keeping it
  that way — a vector must not ship anything that lets its holder produce a
  Decision a verifier would accept as real
- Relates to: `docs/interop/scientific-state-profile-v1.md` contract 4, which
  states this gap about itself

## Context

The interop profile names seven contracts and pairs each with the check that
decides whether an implementation satisfies it. Contract 4, Authority, has no
check, and the profile says so in its own words:

> **This contract has no language-independent conformance vector, and that is a
> gap rather than an omission.** `crates/vela-cli/tests/review_acceptance.rs`
> executes a real authorized acceptance over a contiguous record chain, but it
> is Rust and it exercises this implementation. Contracts 1, 3 and 7 each have
> fixtures a foreign implementation can be held to; this one does not, so an
> implementation claiming to verify authority is currently taking its own word
> for it.

This is confirmed at the code level, not only in prose. `conformance/verify.py`
runs `verify_canonical_hashing.py`, `verify_current_objects.py`,
`verify_wire_schemas.py`, `verify_correction_impact.py`, an inline exact-witness
floor, and the ecosystem check. Nothing exercises an authority chain.

`conformance/fixtures/epoch1/authorization-profile-parity-v1.json` looks like it
might. It does not: its own README records that its test was removed with the
epoch-1 reader, and nothing in `crates/`, `conformance/`, `scripts/` or
`.github/` reads it. `AGENTS.md` records the evaluator half of the same gap.

The asymmetry matters more here than anywhere else in the profile. Producing and
verifying are unprivileged; deciding is not. Contract 4 is the one that says only
a signed Decision by the repository authority moves Standing — the strongest
claim the protocol makes — and it is the only one a foreign implementation can
claim to satisfy without being checked. An implementation that silently accepted
an unsigned Decision, a chain with a gap, or a chain rooted at a trust anchor it
had taken from the repository it was verifying would pass every check the
profile currently has.

What an implementation must do is already written down:

- verify a `vela.authority-record.v1` chain from the sequence-one record; and
- obtain that sequence-one root through a channel other than the repository it
  is verifying (`vela authority trust pin`).

And the shape is fixed. `AuthorityRecordContentV1`
(`crates/vela-protocol/src/kernel/authority.rs:450`) carries `repository_id`,
`sequence`, `previous_authority_record_root`, the before and after event-log
roots, the principal and authentication claims, `authority_keyset_root`, and
`recorded_at`. The record id is `var_` plus the first 16 hex of the canonical
digest of the content. A chain is checkable from those fields alone.

## The question this ADR exists to settle

A vector for contracts 1 and 3 hands a foreign implementation a signing seed and
asks it to produce byte-identical output. `conformance/current-objects/` ships
`producer.seed.hex` and `verifier.seed.hex` for exactly that, and
`verify_current_objects.py` copies each to a temporary file at `0o600` before
handing it to both emitters. Two published Ed25519 seeds already live in this
repository.

The obvious move is to add `authority.seed.hex` beside them. **The question is
whether authority is different in kind, and the answer is not obviously no.**

The argument that it is different: a producer seed lets its holder author a
Submission, and a Submission asserts nothing about Standing — anyone may write
one and it is refused or accepted on its merits. An authority seed lets its
holder sign a record that says a Decision happened. If any verifier anywhere
trusts a key because it appears in a published Vela artifact, that seed is a
skeleton key for a fabricated repository history.

The argument that it is not different: trust in Vela is pinned, never inferred.
A verifier does not accept an authority record because it is signed; it accepts
it because it chains to a sequence-one root obtained out of band. A fixture key
that no one has pinned authorizes nothing, exactly as a fixture producer key
authors nothing anyone accepted. On that reading, a published authority seed is
no more dangerous than the two already here.

The disagreement is not really about cryptography. It is about whether every
downstream implementation will honour the pin. The protocol's own threat model
says trust is pinned; the risk is an implementation that gets it wrong, and a
published seed turns that implementation's bug into someone else's forged
history.

## Decision

Not taken here. Three options, and the third exists because of the question
above.

### Option A — a fixture authority seed, matching contracts 1 and 3

Ship `conformance/authority/` holding a seed, a sequence-one record, a short
contiguous chain, the keyset, and a trust pin. A foreign implementation is
asked to verify the chain and to reproduce the roots.

Simplest, most symmetrical, and directly reuses the machinery
`verify_current_objects.py` already has. It publishes an authority signing key,
which is the thing this ADR is uncertain about.

Mitigations available if this is chosen: a `repository_id` reserved for fixtures
and refused by `vela init`; a `recorded_at` far in the past; a `README` in the
directory stating plainly what the key is and is not; and a keyset marked
`closed`, which the protocol already supports (`AuthorityKeysetV1.closed`) and
which makes the fixture chain terminal by construction.

### Option B — verification-only vectors, with no private key

Ship the chain, the keyset, the trust pin and the expected roots, and no seed at
all. The vector asks an implementation to *verify*, which is the direction the
profile actually cares about: contract 4 constrains consumers, and the profile
says outright that "an implementation that produces Vela state does not need
authority at all".

Covers the whole consumer obligation and the negative cases with it — a chain
with a `sequence` gap, a record whose `previous_authority_record_root` does not
match its predecessor's root, a signature that does not verify, a chain rooted
at an anchor other than the pinned one, and a `closed` keyset asked to sign
again. Those negatives are where an implementation taking its own word for it
actually fails, and none of them needs a private key.

What it does not cover is producing an authority record. Nothing outside this
repository has asked to, and AGENTS.md is explicit that a second implementation
is the evidence for an abstraction.

### Option C — Option B now, Option A when a producer exists

Ship the verification vectors, and record that the signing half is deliberately
absent until some implementation other than this one needs to write an authority
record. If that never happens, the gap was never real.

## Evidence that would settle it

- **Does any prospective consumer need to produce authority records, or only to
  verify them?** If only to verify, B and C cover the contract completely and
  the key question does not arise. This is answerable by asking, and no such
  consumer exists today.
- **Do the negative cases catch a real implementation?** The cheapest test of
  this ADR's premise is to write the negative vectors and run this
  repository's own reader against them. If `conformance/readers/python` does not
  currently reject a gapped chain, the gap is not hypothetical and the
  verification vectors pay for themselves before any foreign implementation
  exists.
- **Whether a fixture key can be made unusable by construction.** If a `closed`
  keyset and a reserved `repository_id` make a fixture chain terminal in a way
  the kernel enforces rather than merely documents, Option A's objection
  weakens considerably. That is checkable against
  `crates/vela-protocol/src/kernel/authority.rs` today.

## Consequences

Whichever is chosen, `docs/interop/scientific-state-profile-v1.md` must keep
stating contract 4's coverage exactly. It currently overstates nothing, which is
why this ADR could be written from the document itself, and that is the property
worth preserving: a profile that claimed a check it did not have would be worse
than the gap.

If any option ships, `conformance/verify.py` gains the check and
`conformance/fixtures/epoch1/authorization-profile-parity-v1.json` should be
addressed at the same time — it is a record nothing reads, sitting in a fixture
directory, in the same subject area, and leaving it there beside a real vector
invites it to be mistaken for one.

If none ships, that is a decision too, and it belongs in the profile beside the
gap: the contract is verified by this implementation's Rust tests and no foreign
implementation is held to it, deliberately.

## Alternatives rejected

**Point the profile at `review_acceptance.rs` and call it covered.** It is an
excellent test and it is the reason the authority argument is checked at all,
but it is Rust exercising this implementation. A conformance vector's whole
purpose is to be checkable by something that shares no code with the reference,
and naming an in-tree test as the check would be the overstatement this document
exists to avoid.

**Revive `authorization-profile-parity-v1.json` as the vector.** It pins the
epoch-1 repositories' commits and is a record rather than a check; its own
README says the test was removed with the epoch-1 reader. It also describes
`frontier_id` and `resource_type: "frontier"`, which is the shape those
repositories genuinely have and not the shape a current implementation must
satisfy. Rewriting it to the current spelling would assert a shape the measured
data does not have, which that README already refuses to do.

**Generate the vector from the live `vela-science/math` authority.** Its chain
is real signed history and its trust root is pinned by real consumers. A
conformance fixture must be freely redistributable and safe to tamper with in
tests, and the negative cases require deliberately corrupted records — which is
exactly what must never exist alongside a live authority's real ones.
