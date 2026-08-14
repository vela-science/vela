# ADR 0042: Policy-bundle rotation, and what it takes to retire Cedar

- Status: Superseded by ADR 0035, 2026-08-09
- Supersession: none of the three options was taken and the rotation writer was
  never written, because ADR 0035's wire break re-genesises `vela-science/math`
  and genesis mints the authority chain fresh. There is no retained bundle left
  to contradict a Cedar-free reader, so the reader could change first and step 3
  of the sequence below had nothing to wait for. The mechanism this record
  describes is accurate and the ordering constraint still holds for any live
  repository inside an epoch; what changed is that the epoch does not survive.
  ADR 0035's implementation note for 2026-08-09 carries the reasoning. This
  record is historical.
- Protocol effect: none as written. Implementing it adds a rotation writer and,
  later, changes what `PolicyBundleV1` may declare
- Product effect: `vela authority` would gain its second subcommand
- Authority effect: the rotation is a signed authority transaction on a live
  repository. Only an operator holding the repository authority key can perform
  it; nothing in this repository can
- Relates to: ADR 0035 (commodity encoding and wire contracts),
  `docs/PORTABLE_WAIST_CAMPAIGN.md` Cut C, ADR 0039 (which renamed everything
  this one cannot)

## Context

Three things this repository would like to retire are unreachable from it, and
they are unreachable for the same reason.

`docs/ECOSYSTEM.md` §4 has recorded the shape of it since the epoch change:
what `vela-science/math`'s authority record still holds from before ADR 0039 is
the Cedar entity `Frontier`, the `frontier_administrator` role, and the
StateTarget type `frontier`, "all inside a valid signature". §6 records the
fourth: `cedar-policy` is still a dependency of the active writer while the
closed evaluator is called only from tests.

The enforcement is not incidental, and it is worth reading in one place because
it is spread across four files.

**The bundle names its evaluator, and the reader checks it.**
`PolicyBundleV1` carries `engine`, `engine_version` and `restricted_profile`,
and `PolicyBundleV1::validate` (`crates/vela-protocol/src/kernel/authority.rs:90`)
rejects any bundle whose three fields disagree with the compiled-in
`CEDAR_ENGINE`, `CEDAR_ENGINE_VERSION` and `CEDAR_PROFILE_V1`.
`verify_record_authorization` (`kernel/authority_history.rs:845`) applies the
same test to the `CedarEvaluation` inside each authority record. Both types
carry `#[serde(deny_unknown_fields)]`, so the fields cannot be softened on the
read path either.

**The runtime policy text is hashed and pinned.**
`crates/vela-cli/src/authority_transaction.rs:1631` hashes the schema, policies
and entities the binary is carrying and compares them with the roots in the
retained bundle, refusing the write on any difference:

```text
runtime Cedar schema, policies, or entities differ from the retained policy bundle
```

`crates/vela-cli/src/cli/authority.rs:50` already states the consequence at the
schema literal: editing one character of `entity Frontier` "makes every
subsequent authority write on that repository fail".

**Removing the dependency fails the build.**
`crates/vela-protocol/tests/engine_pin.rs` reads the workspace `Cargo.toml` and
`Cargo.lock` at test time and requires `cedar-policy` to be present and pinned
to exactly `CEDAR_ENGINE_VERSION`.

**And there is no rotation writer.** The Cedar schema declares a `policy_rotate`
action, and `vela authority` exposes one subcommand, `trust pin`. The only
production path that constructs a `PolicyBundleV1` is
`fresh_authority_policy_for_frontier` in `cli/authority.rs`, reached from `vela
init`, and it sets `previous_bundle_root: None`. Genesis mints a bundle; nothing
succeeds one.

`.github/dependabot.yml` reached this conclusion independently, for *bumping*
rather than removing, and its note is the most compact statement of the
mechanism in the repository: "Leaving the constant alone fails `engine_pin`;
moving it invalidates every bundle already signed… Raise this pin by hand, with
the migration, or not at all."

So the ordering is forced. Any change to the evaluator identity or the policy
text must reach the live repository *before* it reaches the reader, or the
repository stops being able to decide anything.

## Decision

Not taken here. What follows is the sequence any of the three retirements has to
follow, and the options for the first step.

### The sequence

1. **Ship a rotation writer.** A `policy_rotate` authority transaction that
   mints a successor `PolicyBundleV1` with `previous_bundle_root` set to the
   current one, signs it with the repository authority key, and records it as an
   authority event. `AuthorityKeysetV1` already models succession this way
   (`previous_keyset_root`, `activation_record_root`), and `PolicyBundleV1`
   already has the `previous_bundle_root` field genesis leaves `None`. The
   protocol anticipated this; the product never grew the verb.
2. **The operator rotates `vela-science/math`.** This needs the authority key in
   a local OpenSSH agent and cannot be done from CI or from this repository.
3. **Only then** may the reader drop what the old bundle declared.

Reversing 2 and 3 is the failure mode this ADR exists to name: a reader that
refuses the retained bundle turns a live authority read-only, and the repair
requires the very writer step that was skipped.

### The first step, three ways

**Option A — a general `vela authority policy rotate`.** A verb that takes the
new policy material and writes the successor bundle. Most useful, largest new
surface, and AGENTS.md is explicit that a second implementation is the evidence
for an abstraction — there is one repository.

**Option B — a migration-shaped one-shot.** A verb that rotates only to the
policy material the current binary carries, with no operator-supplied input. It
covers every case in this ADR — each retirement is "make the retained bundle
match this binary" — and cannot be used to install arbitrary policy. Smaller,
and honest about being a migration rather than a feature.

**Option C — do nothing, and let the bundle be permanent.** Defensible only if
the answer to all three retirements is "never". It is not: ADR 0035 is Proposed
and would move the signature preimage anyway, and Cut C names Cedar removal as a
goal with gates.

### Sequencing against ADR 0035

If ADR 0035's DSSE v2 migration is accepted, it re-signs the authority surface
regardless. Doing the policy rotation in the same cut costs one operator
ceremony instead of two, and one migration document instead of two. That
argues for deciding this ADR and 0035 together rather than in either order.

## Evidence that would settle it

- **Whether a second live authority is coming.** With one repository, Option B
  is sufficient and Option A is speculative surface. With a second, an operator
  needs a general verb.
- **Whether Cut C's parity work is reachable.** Cedar's removal additionally
  needed every historical Allow recomputed against the closed profile. At
  proposal time `evaluate_authorization_v1` existed without a production
  caller and the epoch-1 corpus was read by nothing. The later implementation
  exercised that corpus before deleting Cedar, then removed the temporary
  migration evidence once the current signed-chain vector shipped.
- **What rotating actually costs.** Nobody has performed one. A rehearsal on a
  disposable repository created by `vela init` would establish the real
  ceremony, and would do it without touching the live authority.

## Consequences

Until this is decided, four things stay and should stay described as blocked
rather than as oversights: `cedar-policy` in the dependency tree, the Cedar
entity `Frontier`, the `frontier_administrator` role, and the StateTarget type
`frontier`. `docs/ECOSYSTEM.md` §4 and §6 already say so; this ADR is what they
point at for the mechanism.

`docs/PORTABLE_WAIST_CAMPAIGN.md` Cut C gains a prerequisite it did not name.
Its list is about *evidence* — retain the inputs, recompute the Allows, prove
parity, replay from a clean clone — and all of that could pass while Cedar
remained unremovable, because the blocker is a signature on a live repository
and not a gap in the evidence.

## Alternatives rejected

**Soften the reader.** Accept a bundle whose `engine` disagrees, or make the
fields optional. Both types are `deny_unknown_fields` over signed bytes, so this
means either a schema version or a reader that no longer checks what it claims
to. The check is the reason the pin is trustworthy.

**Rewrite the retained bundle in place.** It is inside a DSSE signature over
canonical bytes. Changing it invalidates the signature, which is the property
the whole authority argument rests on.

**Archive `vela-science/math` and start again with fresh genesis.** This is what
ADR 0039 did to the four epoch-1 repositories, and it worked because they were
being retired anyway. Doing it to the one live authority to avoid writing a
rotation verb would discard real signed history to save a feature, and would
leave the next rotation in exactly the same position.
