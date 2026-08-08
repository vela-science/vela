# Vela terminology

This is the controlled product language for Vela's current pre-1.0 writer.
Historical object names remain valid when describing their exact source era.

## Product contract

> **Vela is version control for scientific state.**

> **Map the territory. Advance one boundary. Leave the next researcher ahead.**

```text
init -> submit -> verify -> decide -> replay
```

Research navigation wraps that exact operator loop:

```text
map -> target -> work -> submit -> verify -> decide -> remap
```

These navigation verbs are not renamed protocol objects:

```text
map       reads exact state, dependencies, disagreement, and gaps
target    selects bounded work from current state
work      happens in any native human or machine environment
submit    retains authenticated bounded evidence and a requested Claim
verify    records a scoped check over exact inputs
decide    lets named local authority accept, reject, or correct
remap     derives current territory, blockers, and next work from the new root
```

`MAP -> ADVANCE -> REMAP` is the shorter product compression. An authorized
transition is named by the objects that carry it — the Decision, its Event, and
the exact before and after roots — and by nothing further. A Git merge, a
verifier result, or a package publication is none of those things.

The operator loop remains:

```text
init -> submit -> verify -> decide -> replay
```

## Canonical current vocabulary

Four boundaries, and they are not the same boundary. `Frontier` used to be all
of them at once, which is why the repositories drifted. A **Repository** exists
because there is a new authority, never because there is a new topic.

| Term | Meaning | Authority effect |
| --- | --- | ---: |
| Repository | One independently clonable Git repository with stable identity, trust root, canonical history, named authority, and correction policy | Authority boundary |
| Source | A declared external system whose native records are observed exactly, never governed | None |
| Problem | A bounded scientific question organizing Claims, Obligations and Targets | None |
| Obligation | One unresolved requirement needed to assess or advance a Claim | None |
| Target | One machine-addressable, bounded unit of work | None |
| Offer | A derived recommendation that a Target is available | None |
| Run | One execution occurrence in a workbench or verifier | None |
| Claim | One exact assertion under explicit scope and conditions | None |
| Claim Record | Canonical record of a Claim revision, conditions, evidence, provenance, and typed relations | None by itself |
| Artifact | Retained bytes or an immutable locator with exact identity and provenance | None |
| Evidence | A typed role played by an Artifact or Observation relative to a Claim | None |
| Submission | The portable producer package, bound to no Repository | None |
| Proposal | An exact candidate transition in Standing, against one Repository | None |
| Verification Record | A verifier's scoped observation over exact inputs under a named method | None |
| Decision | An authorized judgment over one exact Proposal | Defines transition intent |
| Event | The append-only canonical transition record | Replays authorized effect |
| Standing | The deterministic current status derived from valid Events | Resulting state |
| Frontier | The derived boundary of unresolved scientific state around one or more Problems | None, owns nothing |
| Atlas | A projection across Repositories, Sources, Problems and Frontiers | None |
| Observatory | The first Atlas: a removable read-only projection | None |

**A Frontier has no identifier.** It is a query, not an object: the Problems
with open Obligations, or the Claims lacking Verification, or everything drawn
from one Source. Two Frontiers may overlap completely and neither is wrong,
because neither holds Standing. Wanting to mint a stable id for one is the
signal it has stopped being derived.

## What each term already is elsewhere

Most of the table above is not new. Vela is Git-native and review-shaped, and a
reader arriving from GitHub or from scholarly peer review already knows nearly
all of it. Naming the existing concept is not a weakening of the protocol; it
is what lets the small genuinely new part be seen.

| Term | If you already know |
| --- | --- |
| Repository | a Git repository, with a trust root and a named signing authority |
| Source | an upstream you vendor from, pinned to a revision |
| Problem | a milestone, or a Benchling project |
| Target | an issue |
| Offer | an issue surfacing as ready to pick up |
| Run | a workflow run |
| Artifact | a build artifact, or a deposited file |
| Submission | a pushed branch and its package; a submission in peer review |
| Proposal | a pull request, opened against one Repository |
| Verification Record | a check run, with its scope and its nonclaims written down |
| Decision | the merge ruling; a decision in peer review |
| Event | a commit to an append-only authority log |
| Standing | the state an event-sourced reducer derives |
| Atlas | a read model |

**Claim and Evidence are not new, and this document used to say they were.**
A nanopublication is four named graphs — head, assertion, provenance, pubinfo —
where the assertion graph holds one atomic attributed statement, the whole is
content-addressed by a Trusty URI, and the whole is signed. A Claim Record
reaches the same decomposition: the assertion, its evidence and sources as
provenance, the producer identity and DSSE envelope as pubinfo, and a SHA-256
over canonical bytes as the artifact code. Groth et al. published the anatomy in
2010 and Kuhn et al. the signed decentralised model in 2016. The correction
algebra converged too: `CORRECTION_RELATION_KINDS` is exactly
`["corrects", "supersedes"]`, and nanopublications landed on `npx:supersedes`
and `npx:retracts`. That is external validation, not arrival, and
`paper/vela.md` already says so.

The decomposition is the same. The serialisation is not — a nanopublication
assertion is RDF quads with dereferenceable IRIs queryable over SPARQL, where
`ClaimAssertion` is prose plus a kind label.

Two things do survive that comparison:

- **Obligation** — what is still missing before a Claim can be assessed or
  advanced, retained as state rather than left in a review thread. Nothing in
  the nanopublication literature holds it.
- **The four independent axes** — Claim standing, Verification outcome,
  Proposal status, and repository integrity are separately derived and never
  collapsed. A passing check is not an acceptance; an acceptance is not a
  reproduction; a merge is not a scientific Decision.

And one difference in the admission rule, which is the actual contribution and
is narrower than it is tempting to claim. In the nanopublication network the
rule is fixed in the protocol and keyed to the producer: a retraction is valid
only when signed with the key that signed the original, and the default view
hides validly retracted nanopublications. Vela makes the admission rule a named
**repository authority** whose Decisions are the Events that Standing replays,
so a third party can correct someone else's Claim and move its Standing, and the
correction's validity is settled by a declared trust root rather than by key
equality.

That is the whole of it, and it is enough. The novelty is integration around
authority-scoped append, replay, and root-bound next work. It is not the
invention of identifiers, provenance, research packaging, correction metadata,
or structured claims.

## Retired names

These were removed from the controlled vocabulary because each named something
that already had a name, or something Vela does not govern. They are listed so
a reader meeting one in an older document knows it is not a current object.

| Retired | Say instead |
| --- | --- |
| Finding | accepted Claim |
| Frontier Commit | the Decision, its Event, and the before/after roots |
| Review Packet | the Proposal |
| Frontier map | `vela status`, or the Observatory |
| Attempt | the workbench's own run identity, as provenance |
| Registration Record | the signed Submission, which already binds every link it repeated (ADR 0033) |

The Event kinds moved with the words. `finding.asserted`, `finding.noted`,
`finding.superseded`, `finding.retracted` and `attempt.claimed` are now
`claim.asserted`, `claim.noted`, `claim.superseded`, `claim.retracted` and
`target.claimed`. Because an Event id is derived from its content, this changes
the id of every Event that carried an old kind — which is why the repositories are
re-issued from a fresh genesis rather than patched. `EventKind::Other`
round-trips any unrecognized string, so a repository from an older era still
parses; it simply is not a current Frontier.

## Research and evaluation properties

These terms describe bounded system behavior. They are not protocol objects:

| Property | Meaning |
| --- | --- |
| action-complete | Every represented unresolved item yields a fresh exact Target or an explicit blocker, within declared source coverage, compiler, relation, and resource bounds |
| correction-closed | A declared complete relation slice identifies affected, surviving, and repair-required state after a correction |
| inheritance-complete | A cold successor can recover current Standing, decisive evidence, and the next valid action without private maintainer context |
| Frontier closure | State, coverage, Targets, Decisions, corrections, and handoff each close over exact current inputs or fail explicitly |
| compounding | Inherited Frontier state measurably improves later correct scientific work under a matched comparison |

The current **Math Atlas** is the bounded first-party read product over
`vela-science/math`, the one live mathematics authority. A future federated
Atlas is an unearned cross-Frontier concept, not a current global authority or
completeness claim.

## Daily commands

```text
init status next start submit show why review replay reproduce log
```

| Command | Meaning |
| --- | --- |
| `status` | Summarize Frontier identity, integrity, blockers, counts, authority readiness, and one next action |
| `next` | Return ranked Target Offers |
| `start` | Print a write-free briefing for one exact current Target |
| `submit` | Validate and retain a Submission, then create its Proposal |
| `show` | Inspect one current or historical typed object |
| `why` | Explain current standing from exact evidence, verification, Decisions, Events, and corrections |
| `review accept` | Accept one exact Proposal through repository authority |
| `review reject` | Reject one exact Proposal through repository authority |
| `replay` | Validate schemas, roots, signatures, replay, and policy consistency |
| `reproduce` | Rerun retained evidence or a verifier from exact or declared inputs |
| `log` | Read canonical history |

## Required distinctions

```text
submit != accept
submit != verify
verify != accept
publish != accept
Git merge != scientific Decision
reproduce != endorse
correct != erase
```

A successful Submission normally means:

```text
Submission retained; review required.
Accepted scientific state changed: no.
```

A passing Verification Record must name the exact property, method, inputs,
scope, and nonclaims. It changes no standing by itself.

A current correction, supersession, or retraction is a new authenticated
Submission whose requested change binds the exact historical Claim ID and full
Claim Record root. It never edits the prior Submission, Event, Claim Record, or
Decision.

An acceptance must name the exact Proposal, authorized human principal,
Frontier repository authority, Event, and before/after state roots.

## Lifecycle vocabularies

A native workbench may supply its own run or attempt identity as provenance.
That lifecycle remains defined by the source workbench and never becomes Vela
Standing.

Submission, as this vocabulary declares it:

```text
draft retained refused
```

**No Submission has a status.** `vela.submission.v1` has no lifecycle field,
and none of those three words is a status value anywhere in the crates —
`retained` appears only as a display label on the unchanged-error footer. A
Submission is either installed in a repository or it is not. Treat the three
words as intent for a producer-side lifecycle nothing implements yet, and do
not render them.

Proposal:

```text
pending_review accepted rejected withdrawn
```

Proposal status is derived, not stored: `vela.proposal.v1` carries no status
field either, and the read surface computes these four from the covering
authority Events and any `vela.proposal-withdrawal.v1`. `withdrawn` is the
fourth because PROTOCOL.md section 5.5 makes producer-owned withdrawal a real
transition; `vela review list --status` accepts all four plus `all`, and three
of them are live in the Erdős repository today.

Verification outcome, which is a stored field and does match:

```text
pass fail inconclusive error
```

Claim standing, as this vocabulary declares it:

```text
unassessed accepted accepted_with_conditions retracted superseded corrected
```

The CLI emits four of those six and nothing else. `accepted` follows an
accepted Decision on a `claim.add` or `claim.revise`, `retracted` follows one on
a `claim.withdraw`, `superseded` follows a `claim.superseded` Event, and
`unassessed` covers every Claim over which no ruling stands.

Through `0.966.3` it emitted `pending_review`, `rejected`, and `withdrawn` as
well. Those are Proposal statuses, and putting them on this axis crossed the
separation this document exists to hold. The decision, taken deliberately: the
standing axis reads a ruling, not a queue. Undecided, rejected, and withdrawn
by the producer are one fact about the Claim — nothing has ruled it in — and
`unassessed` is the declared word for exactly that.

The distinction those three words carried was not deleted with them. It is a
fact about the Proposal, so it stays on the Proposal axis and travels beside
the standing rather than inside it: `vela why --json` and `vela show --json`
return `standing` and `proposal_status` as separate fields, and `vela show`
names both in its one line of prose. A reader can still tell a Claim a Decision
rejected from one nobody has looked at, by asking the field named for the axis
that answers.

`vela claims` reports the standing axis alone. It reads the repository manifest
and opens no Proposal, so it has no Proposal status to report — the token the
manifest binds each claim list is a Proposal-axis word, but it records list
membership, and on a compacted Frontier the Proposal it refers to is no longer
retained. Restating it as a Proposal's status would assert a Decision that is
not there.

`accepted_with_conditions` and `corrected` stay declared and underived. A
Decision records no conditions; `corrects` is a Claim relation no Decision
reads. Deriving either from what is retained would be inventing semantics, so
nothing emits them until the protocol gives each one an act to derive from.

Nothing retained changed. The four repositories ADR 0039 archived hold every
indexed Claim at `accepted` and every pending list is empty, so this moved no
repository root and no projection root; it changed which word the read surface
says when a Claim has no ruling.

Claim relations, which are two vocabularies wearing one field name. The
correction algebra, closed and authoritative:

```text
corrects supersedes
```

Descriptive relations, which no Decision reads and which move no Standing:

```text
contradicts depends replicates supports synthesized_from
```

**This one does match the repositories, and it is the only lifecycle
vocabulary on this page that does.** All 1,284 relations retained across the
four Frontiers fall in these two sets, and the split is visible in the records
themselves: every `supersedes` sits on a revision-2 Claim submitted through the
correction path, and every descriptive relation sits on a revision-1 Claim
imported from a corpus. The descriptive set is enumerated from those records
rather than declared ahead of them, so it is open — a Frontier may write a kind
it does not name, and that kind gains no authority by being written.

One near-miss spelling is recognised on input and resolves to a canonical name:
`depends_on` reads as `depends`. Producers emit the canonical spelling;
consumers resolve before matching. `revises`, `retracts` and `opposes` were
declared and written into no record; all three are withdrawn.
`conformance/fixtures/claim-relation-vocabulary-v1.json` fixes the alias, and a
test in `vela-protocol` fails when an implementation drifts from it.

Do not read `evidence[].relation` against this vocabulary. It names the role an
Artifact plays for one Claim, not a link between two Claims, and every retained
Claim Record spells it `supports`.

Artifact axes remain separate:

```text
availability: available | unavailable | restricted | unknown
integrity: matched | mismatched | not_checked
locator: immutable | mutable | unavailable | opaque
```

## Predecessor provenance

Imported Claim Records name their exact predecessor object ID, full root,
source era, and Git commit. The current identity is derived from the current
Claim bytes; an old identity or signature is never copied into a current
authentication field.

All predecessor bytes remain retrievable through the epoch's tag, commit,
Git-object manifest, and archive digest. They are not current writer objects.

## Controlled verbs

Read:

```text
status show list why log
```

Work:

```text
next start run resume abandon
```

Evidence:

```text
submit import reproduce
```

Evaluation:

```text
replay verify reproduce
```

Authority:

```text
accept reject correct supersede retract authorize revoke
```

Distribution and recovery:

```text
export publish repair restore rebuild compact
```

Lower-power roles never receive higher-power verbs.

## Product wording

Use:

- “accepted within this Frontier”;
- “Verification passed for the declared computational property”;
- “Claim fidelity has not been assessed”;
- “Proposal rejected; evidence retained”;
- “No accepted scientific-state change”;
- “published, not accepted”; and
- “producer-reported check”.

Avoid:

- `landed finding`;
- `verified truth`;
- `accepted by verifier`;
- `AI approved`;
- `receipt accepted`;
- `confidence score` as standing;
- `immutable truth`;
- `global knowledge graph`; and
- any unqualified use of `verified`, `valid`, `approved`, or `complete`.

## Analysis and interoperability

These are read, package, and adapter concepts. They do not enter the current
writer merely because they are useful:

| Term | Meaning | Authority effect |
| --- | --- | --- |
| Frontier Algebra | A root-bound derivation of support/opposition routes, corrections, cut sets, and repair requirements | None |
| Discovery Calculus | Optional information and decision lenses for choosing research actions | None |
| Semantic package | A content-addressed set of terms, constraints, mappings, fixtures, licenses, and generated interoperability artifacts | None |
| Verification scope | The exact Claim, inputs, method, environment, and property covered by one Verification Record | None |
| Assurance profile | A versioned description of the assurance dimension addressed by a check, its prerequisites, and nonclaims | None |
| Independence disclosure | Retained common-dependency facts across producers or verifiers | None |
| Mapping | A versioned relation between exact package terms with a declared consequence tier | None by itself |
| Bridge | A maintained set of mappings between domains with every premise and scope needed for transport | None by itself |
| Adapter | A replaceable translation from an exact workbench export to a Submission and explicit loss report | None |
| Lens | A rooted view, metric, or action ordering under declared assumptions | None |
| Package | A versioned, content-addressed unit of reusable language, capability, corpus, verifier, mapping, or adapter; publication has no authority effect | None |
| Federated Atlas | A future removable cross-repository navigation concept, distinct from the current first-party Math Atlas above | None |

Mappings state one consequence tier. The default is `discovery`:

```text
discovery
organization
identity
logical_transport
empirical_transport
```

Shared labels, embeddings, graph proximity, `skos:exactMatch`, or
`owl:sameAs` never transport Standing by themselves.

## Epoch rule

> Verify the signed predecessor boundary. Write exactly one current epoch.
> Never rewrite canonical history to make vocabulary cleaner.

See [ADR 0033](adr/0033-direct-submission-lineage-and-registration-retirement.md)
for the current lifecycle and controlled rewrite gate.
