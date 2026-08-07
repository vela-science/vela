# ADR 0039: Repository is the authority boundary; Frontier is derived

- Status: Accepted
- Accepted: 2026-08-06
- Protocol effect: `Frontier` ceases to be an authority object. The authority
  boundary is named `Repository`. Five Event kinds renamed.
- Product effect: five terms removed from the controlled vocabulary; each
  surviving term stated against its established equivalent
- Authority effect: one mathematics repository replaces four; the human
  Decision ceiling is unchanged
- Supersedes: 0025 §2 (naming), 0028 and 0038 (the four-Frontier topology), and
  the `vela-math` prohibition in 0038 §119, `ROADMAP.md` and `CAMPAIGN.md`, but
  only in the narrow sense stated below

## Context

`Frontier` was doing three incompatible jobs at once: an authority and history
boundary, a topic or corpus boundary, and a product navigation slice. The
definition the vocabulary gave it — "bounded scope, stable identity, canonical
history, authority, and correction policy" — is the definition of a Git
repository under a named authority. It is not the definition of a scientific
frontier.

The consequences were structural, not cosmetic.

**The same territory was sliced twice.** `erdos-frontier` held the Erdős problem
corpus including 699 Claims with Formal Conjectures provenance, while
`formal-conjectures-frontier` separately owned Lean-proof Standing over Formal
Conjectures — including Erdős results. One axis was the problem source, the
other the formalization source. Erdős 521 is the same science in both. A corpus
cannot be the authority over every question it happens to contain.

**Four authorities existed where there is one.** All four repositories name the
same maintainer and the same decision model. Four trust roots bought no
sovereignty; they bought four Standing universes that cannot see each other.

**The topology was codified.** ADR 0028 and the architecture document declared
the four repositories canonical, so every later addition deepened the mistake.

**And the bulk import was admitted as Standing.** Measured across the four
repositories:

| Repository | Claims | with any evidence | Submissions | Verification Records |
| --- | ---: | ---: | ---: | ---: |
| erdos | 2785 | 23 | 13 | 17 |
| sidon | 40 | 22 | 0 | 0 |
| formal-conjectures | 18 | 7 | 4 | 5 |
| quantum-codes | 6 | 6 | 1 | 2 |

2,675 of erdos's provenance entries are `database_record`, and 1,458 Claims
carry corpus-import provenance and nothing else. Fifty-eight Claims in the whole
ecosystem have evidence; twenty-four Verification Records exist. The Observatory
reported 2,782 accepted Claims. A catalogue row was being presented as
adjudicated scientific state.

The data model was already closer to correct than the repository taxonomy: a
Submission binds no Frontier, which is what makes it portable; Verification
Records bind subjects rather than carrying authority; maps, targets, search and
graphs are already explicitly non-authoritative.

## Decision

**1. Separate the four boundaries that `Frontier` was conflating.**

```text
Repository  authority boundary      Git repository + identity + trust root
                                    + authority + canonical history + Standing
Source      provenance boundary     exact native observations of external science
Problem     scientific question     bounded, with its own unresolved boundary
Frontier    derived boundary        the unresolved scientific state around one
                                    or more Problems; owns nothing
Atlas       projection              across repositories, sources, problems
                                    and frontiers
```

**2. A repository exists because there is a new authority, never because there
is a new topic.** This is the rule whose absence produced every item above.

**3. `Frontier` becomes derived and loses its identifier.** A derived boundary
is addressed by its query, not by a minted id. It is the *Frontier* that has no
id; the repository keeps one. `frontier.toml` becomes `vela.toml`,
`frontier_id` becomes `repository_id`, `--frontier` becomes `--repo`. No
aliases: this is one pre-1.0 epoch change and the old machinery is deleted.

The repository mints `repository_id` matching `^vrepo_[0-9a-f]{16}$`. Four
letters rather than the three every other prefix uses, because both three-letter
forms transpose into prefixes already in service — `vrp_` into `vpr_`
(Proposal), `vre_` into `ver_` — and `vre_` is additionally the canonical
wrong-prefix fixture at `objects/current_repository.rs:449`. A misread of the
top-level identity is expensive enough to justify breaking the shape.
`docs/ROOTS.md` states that shape and is rewritten accordingly. `vfr_` is not
reused: every `vfr_` value in existence names an epoch-1 repository and stays
bound to it.

`vela.status.v3` becomes `vela.status.v4`, forced by three shape changes landing
in one document.

**4. There is one derived noun, not two.** A topic lens (Erdős, OEIS,
Lean-formalized) and a state lens (open problems, needs verification, needs
statement-fidelity review) are the same kind of object: a saved query over
Problems, Claims and Obligations that owns no Standing and may overlap any
other. Both are Frontiers. A separate `Collection` term would reintroduce, one
level down, the split this ADR exists to remove.

**5. Remove five terms from the controlled vocabulary.**

| Retired | Say instead |
| --- | --- |
| Finding | accepted Claim |
| Frontier Commit | the Decision, its Event, and the before/after roots |
| Review Packet | the Proposal |
| Frontier map | `vela status`, or the Atlas |
| Attempt | the workbench's own run identity, as provenance |

**6. State every surviving term against what it already is.** Repository is a
Git repository; Target is an issue; Proposal is a pull request; Verification
Record is a check run; Decision is the merge ruling; Event is a commit to an
append-only authority log; Standing is what an event-sourced reducer derives.
Naming the borrowed concept is what lets the unborrowed part be seen: **Claim**,
**Evidence**, **Obligation**, and the four axes that never collapse.

**7. Rename the Event kinds to match.** `finding.asserted`, `finding.noted`,
`finding.retracted`, `finding.superseded` and `attempt.claimed` become
`claim.asserted`, `claim.noted`, `claim.retracted`, `claim.superseded` and
`target.claimed`. `EventKind::Other` still round-trips any unrecognized string,
so an older repository parses and remains readable as history.

**8. Epoch-1 repositories are quarantined, not aliased.** Every canonical object
carries `#[serde(deny_unknown_fields)]`, and `CurrentRepositoryV4::parse`
re-serializes and compares bytes, so a `#[serde(alias)]` would still fail the
canonical-bytes check. A compatibility branch inside the current types is
therefore impossible, not merely undesirable. Instead
`crates/vela-protocol/src/epoch1/` holds a byte-for-byte copy of the epoch-1
object types, frozen and never renamed, and a read-only `vela history <path>`
reads them. The current path drops epoch-1 support entirely, which is what lets
"no aliases" hold literally: epoch 1 is a different schema family, not an alias
inside one.

`.vela/operation-journals/` is gitignored and has zero tracked files in every
repository, so it is machine-local state and needs no migration. The epoch-2
binary writes `.vela/operation-journals/repository/` and never reads the old
directory.

**9. The frozen repositories are the second authority.** Consolidating to one
live repository would otherwise make RQ3, evidence level 2, and gate B8 of
`WHITEPAPER_CONTRACT.md` unprovable, because authority containment needs two
authorities. It does not. The frozen repositories have genuinely distinct trust
roots and real signed history, so re-admitting state from them into
`vela-science/math` through Submission → Verification → Decision *is* the
cross-repository transfer experiment: a foreign transition is retained and
checked, the old roots stay as provenance, and Standing changes only on the new
repository's own Decision. The migration and the experiment are one activity.

**10. The epoch-1 projection stays readable.** The Observatory tags the existing
release as epoch 1 and projects `vela-science/math` as epoch 2. Published record
URLs keep resolving. The history is most of what the Observatory is for.

**11. A mathematics authority is not the `vela-math` that was rejected.** ADR
0038 §119, `ROADMAP.md` and `CAMPAIGN.md` each list "a `vela-math` repository"
under what will not be built, in every case beside "competing theorem library",
"universal ontology" and "separate canonical `problems.science` database". What
those documents reject is a Vela-owned rival to Mathlib and a second canonical
database of mathematics. That rejection stands and is not weakened here.

`vela-science/math` is neither. It is one repository under one authority,
holding the Claims that authority has admitted, exactly as
`vela-science/erdos-frontier` did — one instance of Repository rather than four.
It adds no library, no ontology and no second database. The prohibition is
therefore restated rather than lifted: no mathematics library, no universal
ontology, no rival database; one authority, one repository.

## Amendment, same day: §8, §9 and §10 are withdrawn

Sections 8, 9 and 10 above describe preserving the four pre-0039 repositories
inside the running system: a frozen epoch-1 reader, those repositories standing
in as the second authority for the containment claim, and an epoch-tagged
projection that keeps their record URLs resolving.

All three are withdrawn. The repositories are archived instead — their git
history and signatures stay exactly as they are, and nothing in the running
ecosystem reads them. `crates/vela-protocol/src/epoch1/` was built, verified
against all four checkouts, and then deleted; the current path carries no
epoch-1 branch at all.

The text of §8–§10 is left in place rather than rewritten, because it records
what was decided and why the reversal was cheap: the reader was insurance
against a compatibility problem that only exists if the old repositories stay
live, and they do not.

Two consequences follow and are accepted rather than mitigated.

Published record URLs under the four old slugs stop resolving. A single static
notice replaces them; there is no dual-epoch reader.

RQ3, evidence level 2, and gate B8 of `WHITEPAPER_CONTRACT.md` need two live
authorities and there is one. Authority containment becomes future work pending
a second authority, rather than something this migration demonstrates. That is a
real loss and it is not disguised: it is the strongest claim the protocol makes,
and the current evidence programme cannot reach it from a single repository.

## Consequences

- **One live mathematics repository**, `vela-science/math`, with a fresh
  genesis. Erdős, Formal Conjectures, Sidon, additive combinatorics and quantum
  codes become facets of one state rather than competing authorities.
- **The four existing repositories are frozen**, preserved exactly as historical
  Vela repositories. No further architecture work inside them beyond integrity
  fixes. No multi-parent history merge is attempted: Standing is local and
  foreign Decisions are not transported, and inventing a merge to rescue the
  current layout would distort the protocol to accommodate a mistake.
- **The corpus is reclassified, not imported.** The 1,217-problem Erdős
  catalogue, erdosproblems metadata and Formal Conjectures declarations become
  Source observations. Proof files stay native. Lean stays a verifier. Only
  consequential scientific assertions become Claims.
- **Valuable state is re-admitted deliberately**, through ordinary Submission →
  Verification → Decision. Old repository roots remain provenance. There is no
  migration acceptance and no Decision that nobody made.
- **The public count drops by roughly fifty to one**, from 2,782 accepted Claims
  to the few dozen that carry evidence. This is the honest number and the point
  of the exercise; it should not be softened.
- The map is built from Problems and Claims, never from repositories. An Erdős
  view and a Formal Conjectures view overlap freely because neither owns
  Standing.

## Alternatives rejected

- **Rename `Frontier Commit` to `Frontier Transition`.** Rejected: it keeps a
  term for something with no independent effect that three real objects already
  describe.
- **Re-issue the four Frontiers under the same topology.** Rejected: it would
  have rebuilt the wrong ontology on a clean genesis, making the eventual
  correction more expensive.
- **Keep `Frontier` as the repository and invent a new word for the derived
  boundary.** Rejected: `Frontier` is the scientific word and the derived thing
  is the scientific object. The repository is the one that should take the
  boring name.
- **Merge the four histories into the new repository.** Rejected as above.
