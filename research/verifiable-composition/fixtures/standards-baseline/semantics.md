# Standards baseline semantics v0

This document states the trust boundary of the ADR 0004 exact-lock fixture. The
shared `reference/fact_manifest.py` contract is the only executable definition
of dependency standing. No wrapper or unstated script behavior may add
scientific standing.

## Authority

The scientific DSSE envelope is intentionally unsigned. `science.lock` has one
Ed25519 signature generated from RFC 8032 section 7.1 test vector 1, whose seed
and public key are deliberately public. Its key ID and scope are exactly
`fixture:adr004-standards-baseline` and
`internal_fixture_non_authority`. This is a deterministic corruption check, not
a secret, human identity, Vela authority key, or scientific decision. Any
attempt to substitute it for human authority must fail.

The decision signature and `valid` standing inside the fact manifest are fixed
opaque fixture facts copied byte-for-byte from the Vela-profile arm. This
wrapper does not verify them and they are not a human decision.

In production, a fact manifest may say `decision_status = valid` only after a
separate reader has checked the named decision's full content root, signature,
historical scoped authority, and event-log binding. This fixture supplies no
such reader, key, acceptance, or authority. The baseline result therefore
proves representation equality only.

## Correction and delivery

A later state exists only after a caller explicitly delivers a full Git root.
The checked-in bundle is verified and fetched into a disposable bare
repository. Git plumbing derives the merge base, commit trees, and
same/descendant/ancestor/forked relation. Exact committed event sequences
derive the event relation, event-log roots, snapshot roots, and state-document
roots. No relation string supplied by a caller is trusted.

Verified `same` and `descendant` relations remain on the selected lineage.
Verified `ancestor` resolves to `stale`, verified `forked` resolves to
`forked`, and missing, invalid, or unverifiable delivery resolves to
`unresolvable`. The profile performs no network lookup, monitoring, freshness
promise, registry query, or automatic discovery.

Within a verified selected lineage, available evidence plus an accepted
finding, valid decision, and valid verifier is `satisfied`. Missing or invalid
evidence, decision, or verifier material is `unresolvable`. A corrected or
superseded finding is `review_required`. A withdrawn finding, revoked decision,
or revoked verifier is `blocked` for a `hard`, `data`, or `method` premise and
`review_required` for a `soft` or `contextual` premise.

Every projection has `child_truth = not_assessed`, `child_mutation = none`, and
`authority_effect = none`. A parent correction changes only dependency
standing. It never proves the child false, edits its receipt, or creates an
authority event.

## Exact lock

`science.lock` repeats the complete dependency observation, last-seen and
delivered roots, content-bound delivery inspection, later standing, and this
document's byte root. The dependency's parent commit, tree, event-log root, and
snapshot root must exactly equal `last_seen`. It also binds the canonical fact
manifest, canonical in-toto Statement v1 bytes, and unsigned DSSE envelope
bytes. Receipt roots are lexicographically ordered; verifier attachments are
ordered by attachment ID and then full content root. Every JSON artifact is
UTF-8, duplicate-free, compact, recursively key-sorted, and stored with one
final LF. Every security comparison uses a full Git object ID or full SHA-256
root; short handles are routing labels only.
