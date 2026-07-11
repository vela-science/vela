# ADR 0002: Stable-id + revision-digest pairing is deferred; continuity is a back-pointer chain

- Status: Accepted 2026-07-10.

## Context

Findings (`vf_`) pair a stable identity with a computed content hash: the id
survives review, caveat, and confidence events, and `finding_hash()` gives the
exact revision bytes. Every other truth-bearing object collapses the two —
proposals (`vpr_`), activity records (`vrc_`), verifier attachments (`vva_`),
and events (`vev_`) are pure content addresses, so a revision mints a new id
and nothing survives it.

The agentic-VCS ecosystem argues for pairing both identities everywhere
(Jujutsu's Change ID vs Commit ID; Entire's checkpoint id vs the commit hash):
machine-produced work is frequently rewritten, and a reviewer needs to know
that several revisions belong to one logical episode.

## Decision

Defer the object-model change. Do not add stable identities to `vpr_` /
`vrc_` / `vva_` / `vev_` now.

The id semantics of those objects are load-bearing in ways that make this the
same wire-risk class as the deferred EventKind unification:

- `scripts/cross_impl_conformance.py` re-derives `vpr_`/`vev_` preimages
  independently — a preimage change is a spec change plus a second
  implementation change;
- the signed cascade fixtures and the TS/Python reducer parity suite pin
  event identity; touching them means a fixtures-manifest re-sign ceremony;
- the Lean anchors pin pack-id injectivity over the content-addressed form;
- G4 id-integrity derives trust from `derive_id() == id` — a second identity
  field is a second thing that can lie.

No consumer needs cross-revision identity today. When one does, the finding
model is the template to follow.

## The affordance instead: supersedes chains

Continuity across revisions is a back-pointer, the pattern the codebase
already uses three times (`FindingBundle.previous_version`,
`ScientificDiffPack.parent_pack`, `independent_of`):

- `VerifierAttachment.supersedes: Option<String>` — optional, absent on
  legacy records (ids byte-unchanged), set through `with_supersedes` so the
  id still content-addresses the body;
- `ActivityRecord.supersedes: Option<String>` — same shape; external
  emitters set it in the receipt JSON when they revise a record.

A consumer that wants the logical episode walks the chain; the chain is
inspectable, signed content, and cannot be edited after the fact.

## Revisit trigger

Reopen this decision when a real consumer needs cross-revision identity that
chains cannot serve — e.g. a review surface that must address "the current
revision of proposal X" from an external system without walking history, or
a cross-frontier transfer that must cite a proposal across rewrites. Reopen
deliberately: spec + cross-impl + fixtures + Lean anchors move together.
