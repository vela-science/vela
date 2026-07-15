# ADR 0004 Phase 1 current-object gap report

Status: internal fixture, experiment only. No public type, authority change, or
scientific dependency verdict is proposed here.

The focused test is a shape-compatible synthetic aggregate, not a canonical
current-schema frontier. The selected attachment is accepted by the current
Rust attachment parser when available, and the real gate returns the expected
`needs_verification`/G1 result. That does not validate the synthetic aggregate
as canonical Vela state.

## Result

Current Git and Vela objects can carry most of the bytes needed for an exact
dependency observation. They cannot currently support a complete independent
read-only resolution through public porcelain. The reference resolver therefore
has no success state: it either rejects a byte/root mismatch or returns a typed
`unresolvable` result. In particular, committed `frontier.json` is a derived
aggregate, not canonical custody. A self-consistent stale or fabricated
aggregate remains only candidate provenance until it is replayed from `.vela`.

The fixture proves only that the experiment can:

1. reject branch names and abbreviated object names;
2. read regular-file bytes from one full, already-local Git commit and its full
   root tree, ignoring a changed worktree;
3. exercise the current object link pattern
   `finding.add -> finding.asserted -> review.accepted`;
4. refuse shape-valid but byte-wrong roots for the tree, frontier, event log,
   snapshot declaration, finding revision, decision preimage, Receipt,
   attachment, and premise;
5. require the premise bytes to appear as one exact path-and-digest artifact in
   the retained Receipt accepted with the proposal; and
6. bind selected verifier attachments to the current claim text, target,
   content-derived ID, passed outcome, sound method integrity, and absence of
   undischarged hypotheses.

It does **not** prove that the dummy fixture signature is valid, that the actor
had authority, that the human consumed the selected attachments, that a role is
scientifically sound, or that an outside producer can interoperate.

## Exact algorithms used

| Observation field | Phase 1 computation | Current status |
|---|---|---|
| `parent_git_commit` | Lowercase full 40-character SHA-1 or 64-character SHA-256 object name; `git cat-file -t` must say `commit`. No rev expression is accepted. | Representable |
| `parent_git_tree` | The exact `tree` header of that commit; `git cat-file -t` must say `tree`. | Representable |
| `parent_event_log_root` | Vela canonical JSON v1 over events sorted by `id`, with top-level `signature` removed from each event, then SHA-256. | Representable |
| `parent_snapshot_root` | Observation carries `_meta.snapshot_hash`. The reference candidate recomputation removes `_meta`, `_warning`, `events`, `signatures`, and `proof_state`, then applies Vela canonical JSON v1 and SHA-256. | Bytes reproducible for canonical materializations, but not exposed as a normative read-only verdict |
| `finding_revision_root` | Vela canonical JSON v1 over the current finding with `links` replaced by `[]`, then SHA-256. | Representable for the current revision |
| `decision_event_content_root` | Vela canonical JSON v1 over exactly `schema`, `kind`, `target`, `actor`, `timestamp`, `reason`, `before_hash`, `after_hash`, `payload`, and `caveats`, then SHA-256. The `vev_` handle must equal the first 16 digest hex characters. | Representable |
| `decision_signature` | Exact byte-string equality with the retained `review.accepted.signature`. | Representable as bytes; cryptographic and authority validity unresolved |
| `receipt_roots` | SHA-256 over the complete parsed Receipt v1 using the frozen Receipt RFC 8785/JCS canonicalizer, not Vela's older canonical JSON v1. The path must be `records/receipts/sha256/<root>.json`. | Representable; reference implementation currently uses the released Python parser |
| attachment ID | Vela canonical JSON v1 over the attachment with `id` set to `""`; `vva_` plus the first 16 digest hex characters. | Representable |
| attachment full root | Vela canonical JSON v1 over the complete retained attachment, then SHA-256. | Representable |
| `premise_digest` | SHA-256 over raw premise bytes. Exactly one retained Receipt artifact must have the caller-supplied relative path and the same unprefixed digest. | Bytes and Receipt link representable; observation has no direct locator |

`role` is schema-checked only. It is a consumer assertion, not a fact derivable
from the parent.

## Gap classification

### Awkward location

- Events, proposals, findings, and attachments are scanned from the materialized
  aggregate `frontier.json`; there is no single-object read command that returns
  the relevant canonical preimages and links.
- Phase 1 does not replay canonical `.vela` events/proposals and compare the
  resulting materialization. Every otherwise-matching result therefore includes
  `derived_view_not_canonical_state`.
- The only public strict gate is whole-frontier `vela check <path> --strict
  --json`. Integrating it would require materializing regular blobs from the
  exact commit into a read-only temporary directory, yet it would not close the
  named blockers.
- The frontier path and premise path are resolver inputs rather than fields in
  the observation. The Receipt root binds the premise artifact path indirectly,
  but discovery remains caller-driven.

### Missing porcelain

- No read-only command verifies one arbitrary historical decision event's
  Ed25519 signature and answers whether its actor had the required scope at the
  event timestamp.
- `vela check --strict --json` does not return the recomputed Project snapshot
  root, so a client cannot distinguish a normative snapshot check from a mirror
  implementation's byte parity.
- No public read-only command returns the exact verifier-attachment set consumed
  by a decision or evaluates the gate for a caller-selected set.
- There is no narrow read-only command that materializes or checks one aggregate
  against canonical `.vela` custody while returning the exact object roots this
  resolver needs.
- Receipt v1 has frozen Rust and Python parsers, but there is no small Rust
  read-only command that accepts one retained receipt and emits its canonical
  root plus semantic verdict for this resolver. Phase 1 uses the released Python
  implementation and records this as weaker than landing-path parity.
- There is no Vela command that reads a frontier directly from a full Git commit
  without a checkout/materialization step.

### Missing normative semantics

- The public decision event does not normatively say which Receipt artifacts or
  verifier attachments the reviewer actually consumed. The Decision Plan binds
  private consumed-fact roots, but its selected attachment set is not projected
  into a public retained object.
- The dependency roles (`hard`, `soft`, `data`, `method`, `contextual`) have no
  reducer semantics, invalidation rule, or inheritance policy. Treating a role
  as an automatic child-state update would invent authority.
- The observation does not define repository identity or a repository locator.
  A commit/tree pair fixes bytes once a repository is supplied, but not where a
  resolver should obtain those bytes. Phase 1 forbids network discovery.
- The candidate snapshot algorithm mirrors current materialization, but the
  wrapper/defaulting contract is not a published cross-implementation query.
- Phase 1 restricts the authority-shaped link to initial `finding.add` /
  `finding.asserted`. There is no agreed composition rule for later revisions,
  notes, caveats, supersession, retraction, or policy-lane admissions.

### Cannot be represented by current public objects

- A proof that the named human key validly signed this decision and possessed
  the required historical scope, using only public read-only Vela output.
- A proof from the aggregate alone that it is the current replay of canonical
  `.vela` custody rather than a self-consistent stale or fabricated derived view.
- A proof that the human decision consumed exactly the attachment IDs listed by
  the observation. The public decision carries a Decision Plan root reference,
  not the plan's complete consumed-fact preimage.
- An older finding revision when only the current materialized finding body is
  retained. A historical `after_hash` without the corresponding full body is
  not independently resolvable.
- More than one accepted Receipt for an ordinary current proposal; the current
  `vela_submission` link contains one `receipt_root` and one `receipt_path`.
- A useful strict-check integration that both closes a named missing proof and
  guarantees no socket access. Environment flags are not an operating-system
  network sandbox.

## Why Phase 1 does not invoke the registered checker

`registration/phase0.json` freezes the release tag, release commit, executable
asset kind, platform, byte count, and SHA-256. That is the only acceptable
future checker identity; a caller-supplied hash or `_meta.vela_reducer` value is
not a pin.

Phase 1 nevertheless does not run `vela check --strict`. Its JSON output exposes
neither the named historical decision's signature/authority verdict nor the
normative snapshot root, so it closes neither unresolved check. The frozen asset
is also absent from the fixture, and safe integration would need read-only
materialization plus OS-level network isolation. Adding a partial green signal
and several hundred lines of runner code would make the experiment worse. The
negative result is therefore direct:
`unresolvable:authority_snapshot_porcelain_missing`.

## Smallest useful next primitives

Before a complete resolver can return `verified`, Vela would need narrow,
read-only outputs rather than another orchestration layer:

1. an exact event/decision inspection command that emits canonical content root,
   signature validity, signer key identity, historical authority/scope verdict,
   applied-event/finding link, and the retained consumed-fact preimage;
2. a snapshot-root command that returns the canonical recomputation and format
   identifier from a read-only load; and
3. a Receipt v1 inspect command backed by the same Rust parser and canonicalizer
   used at landing.

Those primitives would consolidate existing internal logic. They should not be
added during Phase 1: doing so would cross the ADR gate from current-object
representability into new public protocol and authority semantics.
