# ADR 0040: A producer-declared dependency on `vela.submission.v1`

- Status: Proposed
- Protocol effect: any option but the first changes `vela.submission.v1`, which
  moves the canonical bytes and the signature preimage of every Submission
- Product effect: `vela correction impact` would have edges to traverse on
  repositories built with the current CLI
- Authority effect: none. A declared dependency is retained description; only an
  attributed human Decision changes Standing
- Relates to: ADR 0004 (*Falsify the need for a scientific dependency
  primitive*), whose GO and PIVOT gates this is evidence for and does not close

## Context

`crates/vela-edge/src/analysis/correction_impact.rs` implements
`vela.correction-impact-projection.v1`. It classifies claim-to-claim edges
through a closed rule table:

```text
depends_on  -> hard_dependency
supports    -> support_route
discovery   -> discovery_only
```

From those it partitions `lost_support_routes` from `surviving_support_routes`
and emits rooted repair obligations. It is the mechanism behind the protocol's
central claim about correction — that correcting one Claim identifies exactly
what rests on it and preserves what does not — and it works: the diamond
fixtures under `conformance/fixtures/correction/` run on every CI run through
`conformance/verify_correction_impact.py`.

The edges it traverses are declared vocabulary.
`crates/vela-protocol/src/objects/claim_record.rs` lists `depends` and
`supports` in `DESCRIPTIVE_RELATION_KINDS`, with `depends_on` aliased to
`depends` — ADR 0004 named `depends` the stored wire value and `depends_on` the
derived rendering.

**No producer can declare one.** `vela.submission.v1`
(`crates/vela-protocol/src/objects/submission_v1.rs`) has no relations field.
Under `#[serde(deny_unknown_fields)]`, the only claim-to-claim link a Submission
can carry is `requested_change.target`, and the sole production write path
converts that into exactly one relation:

```rust
("add_claim", None)              => (1, Vec::new(), "claim.add"),
("correct_claim", Some(target))  => (…, vec![ClaimRelation { kind: "corrects",   … }], "claim.revise"),
("supersede_claim", Some(target))=> (…, vec![ClaimRelation { kind: "supersedes", … }], "claim.revise"),
```

`crates/vela-cli/src/current_submission.rs`. The two correction kinds, or an
empty vector. Every `depends` and `supports` edge in the retained corpus came
from the epoch-1 ingest, and those four repositories are archived and unreadable
by this release.

So a repository built with today's CLI can record a correction and cannot record
a cascade. `vela correction impact` reports an empty one, correctly: there is
nothing to traverse. The projection is not wrong, and it is not exercised by
anything a current producer can write.

Two things make this worth deciding now rather than later.

`docs/ROADMAP.md` puts "Complete one correction cascade and one external
producer handoff through the public map" under **Next**. The CLI cannot express
the input to that.

`docs/BREAKTHROUGH_BENCHMARK.md` is the binary test for the protocol-scale
claim, and its decisive scenario requires a correction that produces an "exact
affected set" with "unaffected knowledge and independent support preserved".
That scenario needs at least one non-correction edge to exist. Today, on a
repository this binary wrote, it cannot.

ADR 0004 is the standing position and it is a falsification programme, not a
closed rejection: its decision gates are still open, one marked *GO: add one
minimal invariant* and one *PIVOT: standards-compatible profile and tooling*.
This ADR is the first evidence in that lane produced by driving a real
correction through the protocol rather than by argument.

## Decision

Not taken here. This ADR states the options, what each costs, and what evidence
would settle it. The ruling is the operator's.

### Option A — do nothing, and say so where the reader is

Keep `vela.submission.v1` as it is. Record in `docs/interop/`, `docs/CLI.md` and
`docs/ECOSYSTEM.md` that the correction projection traverses edges the write
path cannot author, so a conforming implementation knows the surface is
derivation-only.

Costs nothing and moves no bytes. Leaves the roadmap item and the benchmark's
decisive scenario unreachable, and leaves a built, conformance-tested projection
with no producer-reachable input. ADR 0004's gates stay open on the same
evidence they have now.

This is the current state, and the documentation half of it has already landed —
`docs/interop/scientific-state-profile.md` states the limit and this
repository's `CHANGELOG.md` records it as known and unclosed. Choosing A means
accepting that as the end state rather than as a way-point.

### Option B — a `relations` array on the Submission

Add to `vela.submission.v1`:

```json
"relations": [
  { "kind": "depends", "target_claim_id": "vcl_…", "target_claim_root": "sha256:…" }
]
```

The kinds are the existing `DESCRIPTIVE_RELATION_KINDS`; the write path copies
them onto the Claim Record beside any correction relation it already derives.
Nothing about acceptance changes: `moves_standing` reads only
`CORRECTION_RELATION_KINDS`, and a declared dependency stays retained
description.

The cost is exact and large. `vela.submission.v1` is signed over its canonical
bytes, so adding a field changes the preimage. Every conforming emitter —
`conformance/emitters/javascript.mjs`, `conformance/emitters/python.py` — and
every fixture under `conformance/current-objects/` moves with it. Under
`#[serde(deny_unknown_fields)]` there is no additive path: an old reader refuses
a new Submission and a new reader refuses nothing, so this is a version, not an
extension.

Whether that requires `vela.submission.v2` or can ride an existing version bump
is the substantive sub-question, and it interacts with ADR 0035: if the DSSE v2
payload migration is going to move the preimage anyway, doing both in one cut
costs one migration instead of two. ADR 0035 is still Proposed.

**Whether to require `target_claim_root` is the second sub-question.** Requiring
it makes a dependency exact and root-bound, which is what every other reference
in the protocol is, and makes it impossible to declare a dependency on a Claim
that does not yet exist. Omitting it allows forward declaration and gives up
exactness. ADR 0007 (*Full-digest claim revision references*) put the same
question to revision references and came down on requiring the digest; it is
Deferred with its entry gate unmet, so it is a prior reading rather than a
settled precedent.

### Option C — a separate signed relation object

A `vela.claim-relation.v1`, authored and signed independently of the Submission
that created either endpoint, admitted through its own Proposal.

This is the only option that lets a dependency be declared *after* both Claims
exist, which is when a researcher usually knows about it — a producer often
cannot know what its result will end up resting on at the moment it submits.
It also leaves `vela.submission.v1` untouched, so no canonical bytes move.

The cost is a new object type with its own lifecycle: who may author an edge
between two Claims they do not own, what a Decision on a bare relation means,
and whether retracting one is a correction. That is a materially larger protocol
surface than Option B, and AGENTS.md's rule applies — a second implementation is
evidence for an abstraction, and there is one consumer.

## Evidence that would settle it

- **One real correction that should have cascaded and could not.** The
  `vela-science/math` Erdős 321 correction is the closest case to date and did
  not need an edge. A second correction whose affected set is non-empty in fact
  but empty in the projection is the direct evidence, and it distinguishes A
  from B and C on measurement rather than on argument.
- **Whether the edge is knowable at submission time.** If producers who have one
  can state it when they submit, Option B is enough. If they routinely learn it
  later, Option B ships a field that stays empty and Option C is the honest
  shape. This is answerable by asking the producers of the next several
  Submissions and recording what they say.
- **ADR 0035's ruling.** If DSSE v2 is accepted, Option B's cost drops to
  approximately zero marginal cost. If it is rejected or deferred indefinitely,
  Option B has to justify a preimage change on its own.

## Consequences

Whichever is chosen, one thing should stop: `docs/` must not describe correction
cascades as a current capability of a repository built with this CLI. The
correction *primitive* is real and exercised; the *cascade* is a projection over
edges no current producer can author. The two are stated separately in
`docs/ECOSYSTEM.md` §7 today and should stay that way until this is ruled on.

If Option A is chosen, `conformance/fixtures/correction/` stays the only place
the projection is exercised, and that should be said in the fixture directory
rather than inferred from its contents.

If B or C is chosen, `vela correction impact` needs a test over a repository the
test itself builds and corrects, not only over the diamond fixtures — the defect
this whole area produced once already was a projection that had never run
against a repository.

## Alternatives rejected

**Infer dependencies from artifacts.** Two Claims citing the same artifact root
is not a dependency; it is a shared input. Deriving one from the other would
manufacture a scientific relation the producer never asserted, which is the
error the whole Submission boundary exists to prevent.

**Let `vela-edge` write the edges.** It is the analysis layer. `docs/ECOSYSTEM.md`
§8 requires that nothing above the kernel change Standing and that `vela-edge`
be deletable without affecting replay. An edge authored there would be either
authoritative — violating the first — or invisible to replay, which makes it
useless for a cascade.

**Widen `DESCRIPTIVE_RELATION_KINDS`.** The vocabulary is not the constraint.
`depends` and `supports` are already in it; the missing thing is a way for a
signed producer statement to carry one.
